use gst::prelude::*;
use gstreamer as gst;
use std::{
    cell::Cell,
    collections::HashMap,
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
};

#[derive(Debug)]
pub enum PlaybackEvent {
    EndOfStream,
    DurationChanged,
    ClockLost,
    Spectrum(Vec<f32>),
    Error(String),
}

pub struct PlaybackEngine {
    pipeline: gst::Pipeline,
    events: Receiver<PlaybackEvent>,
    _bus_watch: gst::bus::BusWatchGuard,
    loaded: Cell<bool>,
    volume: Cell<f64>,
    request_headers: Arc<Mutex<HashMap<String, String>>>,
}

impl PlaybackEngine {
    pub fn new(initial_volume: f64) -> Result<Self, String> {
        gst::init().map_err(|error| format!("Could not initialize GStreamer: {error}"))?;

        // Deliberately use the legacy `playbin` element instead of GtkMediaFile /
        // GstPlay. Track replacement is performed only after the pipeline has
        // reached NULL, so the decoder graph is completely torn down first.
        let element = gst::ElementFactory::make("playbin")
            .build()
            .map_err(|error| format!("Could not create GStreamer playbin: {error}"))?;
        let pipeline = element
            .downcast::<gst::Pipeline>()
            .map_err(|_| "The GStreamer playbin element is not a pipeline".to_string())?;

        // Music files sometimes expose embedded artwork as a video stream. A
        // fakesink prevents GStreamer from creating a video window for it.
        let video_sink = gst::ElementFactory::make("fakesink")
            .build()
            .map_err(|error| format!("Could not create GStreamer video sink: {error}"))?;
        video_sink.set_property("sync", false);
        pipeline.set_property("video-sink", &video_sink);

        // Feed a real 32-band frequency spectrum to the GTK visualizer. The
        // spectrum element is part of gst-plugins-good and posts magnitude
        // lists on the same bus already used for EOS and playback errors.
        let spectrum = gst::ElementFactory::make("spectrum")
            .property("bands", 32_u32)
            .property("threshold", -80_i32)
            .property("interval", 16_666_667_u64)
            .property("post-messages", true)
            .property("message-magnitude", true)
            .property("message-phase", false)
            .property("multi-channel", false)
            .build()
            .map_err(|error| format!("Could not create GStreamer spectrum analyzer: {error}"))?;
        pipeline.set_property("audio-filter", &spectrum);

        let volume = initial_volume.clamp(0.0, 1.0);
        pipeline.set_property("volume", volume);

        // yt-dlp can return HTTP headers that are required by the temporary
        // Googlevideo URL. Apply them to souphttpsrc whenever playbin creates
        // a network source, while leaving local-file sources untouched.
        let request_headers = Arc::new(Mutex::new(HashMap::<String, String>::new()));
        let source_headers = request_headers.clone();
        let _source_setup = pipeline.connect("source-setup", false, move |values| {
            let source = values
                .get(1)
                .and_then(|value| value.get::<gst::Element>().ok())?;
            let Ok(headers) = source_headers.lock() else {
                return None;
            };
            if headers.is_empty() {
                return None;
            }

            if let Some(user_agent) = headers
                .get("User-Agent")
                .or_else(|| headers.get("user-agent"))
            {
                if source.find_property("user-agent").is_some() {
                    source.set_property("user-agent", user_agent.as_str());
                }
            }
            if let Some(referer) = headers.get("Referer").or_else(|| headers.get("referer")) {
                if source.find_property("referer").is_some() {
                    source.set_property("referer", referer.as_str());
                }
            }

            if source.find_property("extra-headers").is_some() {
                let mut extra = gst::Structure::new_empty("headers");
                let mut has_extra = false;
                for (key, value) in headers.iter() {
                    if key.eq_ignore_ascii_case("user-agent") || key.eq_ignore_ascii_case("referer")
                    {
                        continue;
                    }
                    extra.set(key.as_str(), value.as_str());
                    has_extra = true;
                }
                if has_extra {
                    source.set_property("extra-headers", extra);
                }
            }
            None
        });

        let bus = pipeline
            .bus()
            .ok_or_else(|| "The GStreamer playback pipeline has no message bus".to_string())?;
        let (event_tx, events) = mpsc::channel();
        let bus_watch = bus
            .add_watch_local(move |_, message| {
                use gst::MessageView;

                match message.view() {
                    MessageView::Eos(..) => {
                        let _ = event_tx.send(PlaybackEvent::EndOfStream);
                    }
                    MessageView::DurationChanged(..) | MessageView::AsyncDone(..) => {
                        let _ = event_tx.send(PlaybackEvent::DurationChanged);
                    }
                    MessageView::ClockLost(..) => {
                        let _ = event_tx.send(PlaybackEvent::ClockLost);
                    }
                    MessageView::Element(element) => {
                        if let Some(structure) = element.structure() {
                            if structure.name().as_str() == "spectrum" {
                                if let Ok(magnitudes) = structure.get::<gst::List>("magnitude") {
                                    let values = magnitudes
                                        .iter()
                                        .filter_map(|value| value.get::<f32>().ok())
                                        .collect::<Vec<_>>();
                                    if !values.is_empty() {
                                        let _ = event_tx.send(PlaybackEvent::Spectrum(values));
                                    }
                                }
                            }
                        }
                    }
                    MessageView::Error(error) => {
                        let source = error
                            .src()
                            .map(|source| source.path_string().to_string())
                            .unwrap_or_else(|| "unknown GStreamer element".to_string());
                        let debug = error
                            .debug()
                            .map(|debug| debug.to_string())
                            .filter(|debug| !debug.is_empty())
                            .map(|debug| format!(" — {debug}"))
                            .unwrap_or_default();
                        let _ = event_tx.send(PlaybackEvent::Error(format!(
                            "{}: {}{}",
                            source,
                            error.error(),
                            debug
                        )));
                    }
                    _ => {}
                }

                gst::glib::ControlFlow::Continue
            })
            .map_err(|error| format!("Could not attach the GStreamer bus watch: {error}"))?;

        Ok(Self {
            pipeline,
            events,
            _bus_watch: bus_watch,
            loaded: Cell::new(false),
            volume: Cell::new(volume),
            request_headers,
        })
    }

    pub fn load(&self, uri: &str, autoplay: bool) -> Result<(), String> {
        self.load_with_headers(uri, autoplay, HashMap::new())
    }

    pub fn load_with_headers(
        &self,
        uri: &str,
        autoplay: bool,
        headers: HashMap<String, String>,
    ) -> Result<(), String> {
        self.loaded.set(false);

        // Going all the way to NULL removes pads, decoders and pending bus
        // messages before a new URI is assigned. This is intentionally not a
        // gapless transition: reliability is more important here.
        self.pipeline
            .set_state(gst::State::Null)
            .map_err(|_| "GStreamer could not stop the previous track".to_string())?;

        *self
            .request_headers
            .lock()
            .map_err(|_| "Could not update YouTube HTTP headers".to_string())? = headers;
        self.pipeline.set_property("uri", uri);
        self.pipeline.set_property("volume", self.volume.get());

        let target = if autoplay {
            gst::State::Playing
        } else {
            gst::State::Paused
        };
        self.pipeline
            .set_state(target)
            .map_err(|_| "GStreamer could not start the selected track".to_string())?;
        self.loaded.set(true);
        Ok(())
    }

    pub fn play(&self) -> Result<(), String> {
        if !self.loaded.get() {
            return Ok(());
        }
        self.pipeline
            .set_state(gst::State::Playing)
            .map(|_| ())
            .map_err(|_| "GStreamer could not resume playback".to_string())
    }

    pub fn pause(&self) -> Result<(), String> {
        if !self.loaded.get() {
            return Ok(());
        }
        self.pipeline
            .set_state(gst::State::Paused)
            .map(|_| ())
            .map_err(|_| "GStreamer could not pause playback".to_string())
    }

    pub fn recover_clock(&self) -> Result<(), String> {
        if !self.loaded.get() {
            return Ok(());
        }

        let should_resume = self.pipeline.current_state() == gst::State::Playing;
        self.pipeline
            .set_state(gst::State::Paused)
            .map_err(|_| "GStreamer could not pause after losing its clock".to_string())?;

        if should_resume {
            self.pipeline
                .set_state(gst::State::Playing)
                .map_err(|_| "GStreamer could not resume after recovering its clock".to_string())?;
        }

        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        if !self.loaded.get() {
            return Ok(());
        }
        self.pipeline
            .set_state(gst::State::Ready)
            .map(|_| ())
            .map_err(|_| "GStreamer could not stop playback".to_string())
    }

    pub fn shutdown(&self) {
        self.loaded.set(false);
        let _ = self.pipeline.set_state(gst::State::Null);
    }

    pub fn is_playing(&self) -> bool {
        self.loaded.get() && self.pipeline.current_state() == gst::State::Playing
    }

    pub fn is_seekable(&self) -> bool {
        self.loaded.get() && self.duration_us() > 0
    }

    pub fn position_us(&self) -> i64 {
        if !self.loaded.get() {
            return 0;
        }
        self.pipeline
            .query_position::<gst::ClockTime>()
            .map(|position| position.useconds().min(i64::MAX as u64) as i64)
            .unwrap_or(0)
    }

    pub fn duration_us(&self) -> i64 {
        if !self.loaded.get() {
            return 0;
        }
        self.pipeline
            .query_duration::<gst::ClockTime>()
            .map(|duration| duration.useconds().min(i64::MAX as u64) as i64)
            .unwrap_or(0)
    }

    pub fn seek(&self, position_us: i64) -> Result<(), String> {
        if !self.loaded.get() {
            return Ok(());
        }

        let position = gst::ClockTime::from_useconds(position_us.max(0) as u64);
        self.pipeline
            .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT, position)
            .map_err(|error| format!("GStreamer could not seek: {error}"))
    }

    pub fn set_volume(&self, volume: f64) {
        let volume = volume.clamp(0.0, 1.0);
        self.volume.set(volume);
        self.pipeline.set_property("volume", volume);
    }

    pub fn try_recv(&self) -> Option<PlaybackEvent> {
        self.events.try_recv().ok()
    }
}

impl Drop for PlaybackEngine {
    fn drop(&mut self) {
        self.shutdown();
    }
}
