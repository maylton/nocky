# Performance benchmark targets

Use these targets while validating Home V2 and Queue 2.0 on representative hardware.

- First Home interaction remains responsive while remote artwork is still resolving.
- Late artwork updates do not rebuild the complete mounted Home tree.
- Normal desktop widths use approximately six Home cards per row; compact and wide windows reflow automatically.
- Opening and scrolling a queue with hundreds of tracks does not trigger repeated full-image decoding.
- Queue drag target calculation remains independent from the number of mounted rows.
- Stream preloading remains bounded and yields to explicit playback requests.
- Long playback sessions do not show unbounded artwork texture growth.

Record cold-cache and warm-cache timings separately, along with window size, queue length, CPU usage and peak memory.
