// Runs in the page itself (content script world "MAIN") on https sites,
// before the page's own code. Chrome lets call sites report their
// microphone and camera state and offer "toggle microphone" and "hang up"
// controls through the Media Session API (made for Chrome's floating call
// window). Sites that use it (Google Meet does) tell us, in a standard
// way, whether you are muted and how to mute or leave. This notes what
// the page reports and passes it to session.ts as a DOM event; it can
// also call the page's own handler when the island asks. Sites that do
// not use it are left alone.

const STATE_EVENT = 'selawi-island-session';
const ACTION_EVENT = 'selawi-island-session-action';
const ACTIONS = ['togglemicrophone', 'togglecamera', 'hangup'] as const;
type CallAction = typeof ACTIONS[number];

type Handler = ((details: {action: string}) => void) | null;

interface CallMediaSession {
    setActionHandler(action: string, handler: Handler): void;
    setMicrophoneActive?(active: boolean): Promise<void> | void;
    setCameraActive?(active: boolean): Promise<void> | void;
}

const proto = (window as unknown as {MediaSession?: {prototype: CallMediaSession}}).MediaSession?.prototype;
if (proto && typeof proto.setActionHandler === 'function') {
    let mic: boolean | null = null;
    let camera: boolean | null = null;
    const handlers = new Map<CallAction, NonNullable<Handler>>();
    let last = '';

    const report = (): void => {
        const detail = JSON.stringify({mic, camera, actions: [...handlers.keys()]});
        if (detail === last)
            return;
        last = detail;
        document.dispatchEvent(new CustomEvent(STATE_EVENT, {detail}));
    };

    const isCallAction = (action: string): action is CallAction =>
        (ACTIONS as readonly string[]).includes(action);

    const setActionHandler = proto.setActionHandler;
    proto.setActionHandler = function (this: CallMediaSession, action: string, handler: Handler): void {
        setActionHandler.call(this, action, handler);
        if (isCallAction(action)) {
            if (typeof handler === 'function')
                handlers.set(action, handler);
            else
                handlers.delete(action);
            report();
        }
    };

    const wrap = (name: 'setMicrophoneActive' | 'setCameraActive', set: (active: boolean) => void): void => {
        const original = proto[name];
        if (typeof original !== 'function')
            return;
        proto[name] = function (this: CallMediaSession, active: boolean) {
            const result = original.call(this, active);
            set(active === true);
            report();
            return result;
        };
    };
    wrap('setMicrophoneActive', active => {
        mic = active;
    });
    wrap('setCameraActive', active => {
        camera = active;
    });

    // The island asked for one of the page's own controls.
    document.addEventListener(ACTION_EVENT, event => {
        const action = (event as CustomEvent<unknown>).detail;
        if (typeof action === 'string' && isCallAction(action))
            handlers.get(action)?.({action});
    });
}
