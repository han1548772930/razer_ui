# Audio demo playback boundary (2026-10-04)

The three current audio demo roots mount a poster button for a real video with
an audio track. This workspace has no native media player transport, so the
poster cannot produce sound or report playback progress.

The poster action remains keyboard and pointer accessible. On an explicit click
it records a local request and shows `音频演示播放服务未接入`. The source-sized
control bar is present after that request. Its actual current mounted source
disables default controls and includes only play, progress and fullscreen; the
previous volume, mute and percentage preview were unsupported additions and
have been removed. The bar uses the source 40px height, zero padding, 5px radius
and translucent background. Its play/fullscreen outlines come from the current
product CSS's embedded font. Media controls remain disabled until an audible
transport is connected; they do not simulate time, playback or a successful
service response. The independent floating preference remains editable.

This is a real functionality gap: source play/pause/seek events, elapsed time,
duration, looping, fullscreen and the floating window require the media player.
It is separate from deferred DLL device writes. Detailed source receipts,
remaining progress geometry and runtime limits are recorded in
[the current audit](audio-demo-current-audit.md).
