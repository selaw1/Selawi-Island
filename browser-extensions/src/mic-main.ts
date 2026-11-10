// Runs in the page itself (content script world "MAIN") on https sites,
// before the page's own code. Linux only sees "the browser is using the
// microphone"; this notices which site it is: whether the page holds a
// live microphone track from getUserMedia. The answer (true/false) goes
// to session.ts as a DOM event. No audio is read.

const MIC_EVENT = 'selawi-island-mic';
const MIC_CHECK_MS = 2000;

const devices = window.MediaDevices?.prototype;
if (devices && typeof devices.getUserMedia === 'function') {
    const tracks = new Set<MediaStreamTrack>();
    let active = false;
    let timer: ReturnType<typeof setInterval> | null = null;

    const check = (): void => {
        for (const track of [...tracks]) {
            if (track.readyState === 'ended')
                tracks.delete(track);
        }
        const now = tracks.size > 0;
        if (now !== active) {
            active = now;
            document.dispatchEvent(new CustomEvent(MIC_EVENT, {detail: now}));
        }
        if (!now && timer) {
            clearInterval(timer);
            timer = null;
        }
    };

    const getUserMedia = devices.getUserMedia;
    devices.getUserMedia = async function (this: MediaDevices, constraints?: MediaStreamConstraints) {
        const stream = await getUserMedia.call(this, constraints);
        for (const track of stream.getAudioTracks()) {
            tracks.add(track);
            track.addEventListener('ended', check);
        }
        // A page stops a track with track.stop(), which fires no event:
        // look again now and then, only while a microphone is held.
        timer ??= setInterval(check, MIC_CHECK_MS);
        check();
        return stream;
    };
}
