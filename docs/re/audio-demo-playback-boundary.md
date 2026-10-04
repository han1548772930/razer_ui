# Audio demo playback boundary (2026-10-04)

The three current audio demo roots mount a poster button for a real video with
an audio track. This workspace has no native media player transport, so the
poster cannot produce sound or report playback progress.

The poster action remains keyboard and pointer accessible. On an explicit click
it records a local request and shows `音频演示播放服务未接入`. The source-sized
control bar is now present after that request: a local progress slider, volume
slider, mute/play glyphs, and fullscreen affordance preserve the video-react
layout. The sliders only retain preview values in the page entity; they do not
simulate progress, audio output, floating video, or a successful service
response. A future native player can replace this local control state while
retaining the audited poster, dimensions, and floating preference.
