// Runs in the page itself (content script world "MAIN") on https sites,
// before the page's own code. Web calls (Slack, Teams, Zoom, Discord,
// WhatsApp…) all use the browser's RTCPeerConnection. This wraps it to
// notice one thing only: is a live call connection sending your
// microphone? The answer (true/false) goes to call.ts as a DOM event.
// Nothing about the call's content, people or audio is read.

const EVENT = 'selawi-island-call';
const CHECK_MS = 2000;

const Native = window.RTCPeerConnection;
if (typeof Native === 'function') {
    const connections = new Set<RTCPeerConnection>();
    let inCall = false;
    let timer: ReturnType<typeof setInterval> | null = null;

    const sendsMicrophone = (pc: RTCPeerConnection): boolean => {
        const connected = pc.connectionState === 'connected' ||
            pc.iceConnectionState === 'connected' || pc.iceConnectionState === 'completed';
        return connected && pc.getSenders().some(s => s.track?.kind === 'audio' && s.track.readyState === 'live');
    };

    const check = (): void => {
        for (const pc of [...connections]) {
            if (pc.connectionState === 'closed')
                connections.delete(pc);
        }
        const now = [...connections].some(sendsMicrophone);
        if (now !== inCall) {
            inCall = now;
            document.dispatchEvent(new CustomEvent(EVENT, {detail: now}));
        }
        if (connections.size === 0 && timer) {
            clearInterval(timer);
            timer = null;
        }
    };

    class WatchedPeerConnection extends Native {
        constructor(configuration?: RTCConfiguration) {
            super(configuration);
            connections.add(this);
            this.addEventListener('connectionstatechange', check);
            this.addEventListener('iceconnectionstatechange', check);
            // Tracks can be added or stopped after connecting: look again
            // now and then, only while a connection exists.
            timer ??= setInterval(check, CHECK_MS);
        }
    }

    Object.defineProperty(window, 'RTCPeerConnection', {value: WatchedPeerConnection, writable: true, configurable: true});
    if ('webkitRTCPeerConnection' in window)
        Object.defineProperty(window, 'webkitRTCPeerConnection', {value: WatchedPeerConnection, writable: true, configurable: true});
}
