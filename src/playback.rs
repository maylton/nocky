use gstreamer as gst;
use gst::prelude::*;
use std::{
    cell::Cell,
    sync::mpsc::{self, Receiver},
};

#[derive(Debug)]
pub enum PlaybackEvent {
    EndOfStream,
    DurationChanged,
    Spectrum(Vec<f32>),
    Error(String),
}

pub struct PlaybackEngine {
    pipeline: gst::Pipeline,
    events: Receiver<PlaybackEvent>,
    _bus_watch: gst::bus::BusWatchGuard,
    loaded: Cell<bool>,
    volume: Cell<f64>,
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
            .property("interval", 50_000_000_u64)
            .property("post-messages", true)
            .property("message-magnitude", true)
            .property("message-phase", false)
            .property("multi-channel", false)
            .build()
            .map_err(|error| format!("Could not create GStreamer spectrum analyzer: {error}"))?;
        pipeline.set_property("audio-filter", &spectrum);

        let volume = initial_volume.clamp(0.0, 1.0);
        pipeline.set_property("volume", volume);

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
                    MessageView::Element(element) => {
                        if let Some(structure) = element.structure() {
                            if structure.name().as_str() == "spectrum" {
                                if let Ok(magnitudes) = structure.get::<gst::List>("magnitude") {
                                    let values = magnitudes
                                        .iter()
                                        .filter_map(|value| value.get::<f32>().ok())
                                        .map(|decibels| {
                                            // Ignore the quietest floor and keep loud tracks from
                                            // pinning every band at full height.
                                            (((decibels + 66.0) / 66.0)
                                                .clamp(0.0, 1.0)
                                                .powf(1.12)
                                                * 0.78)
                                                .min(0.86)
                                        })
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
        })
    }

    pub fn load(&self, uri: &str, autoplay: bool) -> Result<(), String> {
        self.loaded.set(false);

        // Going all the way to NULL removes pads, decoders and pending bus
        // messages before a new URI is assigned. This is intentionally not a
        // gapless transition: reliability is more important here.
        self.pipeline
            .set_state(gst::State::Null)
            .map_err(|_| "GStreamer could not stop the previous track".to_string())?;

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
