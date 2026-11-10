// Loads the real built extension into Chromium, serves a stand-in Google
// Meet page (same buttons Meet uses: Material icon names and
// data-is-muted), and checks the whole path through the real native host
// and daemon. Run by browser-e2e.sh.
import {execFileSync} from 'node:child_process';
import {mkdirSync, mkdtempSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {createRequire} from 'node:module';

const require = createRequire(import.meta.url);
const {chromium} = require('playwright');

const BIN = resolve('target/debug');
const EXTENSION = resolve('browser-extensions/build/chrome');
const sleep = ms => new Promise(r => setTimeout(r, ms));
const list = () => JSON.parse(execFileSync(`${BIN}/selawi-island`, ['list'], {encoding: 'utf8'}));
const invoke = (id, action) => execFileSync('gdbus', ['call', '--session', '--dest', 'com.selawi.Island',
    '--object-path', '/com/selawi/Island', '--method', 'com.selawi.Island.Events1.InvokeAction', id, action]);
const ok = (cond, msg) => {
    if (!cond)
        throw new Error(msg);
    console.log(`ok: ${msg}`);
};
async function waitFor(what, check) {
    for (let i = 0; i < 100; i++) {
        const found = check(list());
        if (found)
            return found;
        await sleep(100);
    }
    throw new Error(`timed out: ${what}; events: ${JSON.stringify(list().map(e => [e.id, e.state]))}`);
}

// A tiny real PNG used as each stand-in site's icon.
const FAVICON = '<link rel="icon" href="data:image/png;base64,' +
    'iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAAGUlEQVR4nGN4amn6nxLMMGrAqAGjBgwXAwC1ulIfmdOREQAAAABJRU5ErkJggg==">';

// Like real Meet: a leave button on the page, and microphone / camera
// state and controls through Chrome's Media Session call controls.
const MEET = `<!doctype html><html><head><title>Meet - Weekly sync</title>${FAVICON}</head><body>
<button id="leave" aria-label="Leave call"><i class="google-material-icons">call_end</i></button>
<script>
const session = navigator.mediaSession;
window.meetMuted = false;
session.setActionHandler('togglemicrophone', () => {
  window.meetMuted = !window.meetMuted;
  session.setMicrophoneActive(!window.meetMuted);
});
session.setActionHandler('togglecamera', () => {});
session.setActionHandler('hangup', () => { document.body.innerHTML = '<p>You left the meeting</p>'; });
session.setMicrophoneActive(true);
session.setCameraActive(true);
</script></body></html>`;

// Chromium looks for native host registrations inside its profile folder
// when it runs with its own profile (for the real Chrome it is
// ~/.config/google-chrome/NativeMessagingHosts).
const profile = mkdtempSync(join(tmpdir(), 'selawi-chrome-'));
mkdirSync(join(profile, 'NativeMessagingHosts'));
writeFileSync(join(profile, 'NativeMessagingHosts', 'io.selawi.island.json'), JSON.stringify({
    name: 'io.selawi.island', description: 'Selawi Island', type: 'stdio',
    path: `${BIN}/selawi-island-browser-host`,
    allowed_origins: ['chrome-extension://bjooidhahonkdcdpibfadoapilihpgig/'],
}));

// One second of silence, looped, so the stand-in pages really play.
function silentWav() {
    const rate = 8000, samples = rate;
    const buf = Buffer.alloc(44 + samples * 2);
    buf.write('RIFF', 0); buf.writeUInt32LE(36 + samples * 2, 4); buf.write('WAVE', 8);
    buf.write('fmt ', 12); buf.writeUInt32LE(16, 16); buf.writeUInt16LE(1, 20); buf.writeUInt16LE(1, 22);
    buf.writeUInt32LE(rate, 24); buf.writeUInt32LE(rate * 2, 28); buf.writeUInt16LE(2, 32); buf.writeUInt16LE(16, 34);
    buf.write('data', 36); buf.writeUInt32LE(samples * 2, 40);
    return `data:audio/wav;base64,${buf.toString('base64')}`;
}
const YOUTUBE = `<!doctype html><html><head><title>Midnight City - YouTube</title>${FAVICON}</head><body>
<div class="html5-video-player"><audio loop autoplay src="${silentWav()}"></audio></div>
<button class="ytp-next-button" onclick="document.body.dataset.next='pressed'">next</button>
<button class="ytp-prev-button" style="display:none">prev</button>
<script>navigator.mediaSession.metadata = new MediaMetadata({title: 'Midnight City', artist: 'M83'});</script>
</body></html>`;
const SOUNDCLOUD = `<!doctype html><html><head><title>Night Drive by Artist | SoundCloud</title></head><body>
<div class="playbackTimeline__timePassed"><span aria-hidden="true">1:23</span></div>
<div class="playbackTimeline__duration"><span aria-hidden="true">3:35</span></div>
<button class="skipControl__next">next</button><button class="playControls__play">play</button>
<script>navigator.mediaSession.metadata = new MediaMetadata({title: 'Night Drive', artist: 'Artist'});
navigator.mediaSession.playbackState = 'playing';</script>
</body></html>`;

// A page that makes a real WebRTC call to itself with the fake microphone,
// like any web call (Slack huddle, Teams, Zoom…) does underneath.
const SLACK = `<!doctype html><html><head><title>Slack | huddle</title></head><body><script>
async function startCall() {
  const mic = await navigator.mediaDevices.getUserMedia({audio: true});
  const a = new RTCPeerConnection(), b = new RTCPeerConnection();
  a.onicecandidate = e => e.candidate && b.addIceCandidate(e.candidate);
  b.onicecandidate = e => e.candidate && a.addIceCandidate(e.candidate);
  for (const t of mic.getTracks()) a.addTrack(t, mic);
  await a.setLocalDescription(await a.createOffer());
  await b.setRemoteDescription(a.localDescription);
  await b.setLocalDescription(await b.createAnswer());
  await a.setRemoteDescription(b.localDescription);
  window.hangUp = () => { a.close(); b.close(); mic.getTracks().forEach(t => t.stop()); };
}
startCall();
</script></body></html>`;

// The same call, on a page that also reports its microphone and offers
// mute / hang up through Chrome's Media Session call controls.
const TEAMS = SLACK.replace('<title>Slack | huddle</title>', '<title>Teams</title>').replace('startCall();', `
startCall().then(() => {
  let muted = false;
  navigator.mediaSession.setActionHandler('togglemicrophone', () => {
    muted = !muted;
    navigator.mediaSession.setMicrophoneActive(!muted);
    window.pageMuted = muted;
  });
  navigator.mediaSession.setActionHandler('hangup', () => { window.leftFromIsland = true; window.hangUp(); });
  navigator.mediaSession.setMicrophoneActive(true);
});`);
const DATA_ONLY = `<!doctype html><html><body><script>
const a = new RTCPeerConnection(), b = new RTCPeerConnection();
a.createDataChannel('x');
a.onicecandidate = e => e.candidate && b.addIceCandidate(e.candidate);
b.onicecandidate = e => e.candidate && a.addIceCandidate(e.candidate);
(async () => { await a.setLocalDescription(await a.createOffer()); await b.setRemoteDescription(a.localDescription);
  await b.setLocalDescription(await b.createAnswer()); await a.setRemoteDescription(b.localDescription); })();
</script></body></html>`;

const context = await chromium.launchPersistentContext(profile, {
    headless: false,
    acceptDownloads: true,
    args: [`--disable-extensions-except=${EXTENSION}`, `--load-extension=${EXTENSION}`,
        '--autoplay-policy=no-user-gesture-required',
        '--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream'],
    permissions: ['microphone'],
});
try {
    await context.route('https://meet.google.com/**', route =>
        route.fulfill({status: 200, contentType: 'text/html', body: MEET}));
    await context.route('https://www.youtube.com/**', route =>
        route.fulfill({status: 200, contentType: 'text/html', body: YOUTUBE}));
    await context.route('https://soundcloud.com/**', route =>
        route.fulfill({status: 200, contentType: 'text/html', body: SOUNDCLOUD}));
    await context.route('https://app.slack.com/**', route =>
        route.fulfill({status: 200, contentType: 'text/html', body: SLACK}));
    await context.route('https://teams.microsoft.com/**', route =>
        route.fulfill({status: 200, contentType: 'text/html', body: TEAMS}));
    await context.route('https://p2p.example/**', route =>
        route.fulfill({status: 200, contentType: 'text/html', body: DATA_ONLY}));
    await context.route('https://files.example/**', route =>
        route.fulfill({status: 200, body: Buffer.alloc(256 * 1024),
            headers: {'content-type': 'application/octet-stream', 'content-disposition': 'attachment; filename="report.pdf"'}}));

    const page = await context.newPage();
    await page.goto('https://meet.google.com/abc-defg-hij');

    const call = await waitFor('Meet call', events => events.find(e => e.id.startsWith('browser:meet:') &&
        e.state !== 'ended' && e.payload.muted !== undefined));
    ok(call.payload.title === 'Weekly sync', 'the call appears with the meeting name');
    ok(call.payload.confidence === 'confirmed' && call.payload.muted === false, 'confirmed call, microphone on');
    ok(call.payload.camera === true, 'the camera state comes from Meet\'s call controls');
    const withLogo = await waitFor('Meet logo', events => events.find(e => e.id === call.id && e.source.icon));
    ok(withLogo.source.icon.startsWith('data:image/png;base64,'), 'the call carries the site\'s logo as a clean PNG');
    ok(call.actions.map(a => a.id).join() === 'mute,leave', 'Mute and Leave buttons');

    invoke(call.id, 'mute');
    await waitFor('muted', events => events.find(e => e.id === call.id && e.payload.muted === true));
    ok(await page.evaluate(() => window.meetMuted) === true, 'Mute in the island used Meet\'s own mute');

    invoke(call.id, 'unmute');
    await waitFor('unmuted', events => events.find(e => e.id === call.id && e.payload.muted === false));
    ok(true, 'Unmute works the same way');

    invoke(call.id, 'leave');
    await waitFor('call ended', events => events.find(e => e.id === call.id && e.state === 'ended'));
    ok(await page.locator('#leave').count() === 0, 'Leave used Meet\'s hang up and the call ended');

    const download = page.waitForEvent('download');
    await page.evaluate(() => { location.href = 'https://files.example/report'; });
    await (await download).path();
    const shown = await waitFor('download', events => events.find(e => e.id.startsWith('browser:download:')));
    // Playwright saves downloads under random ids, so the real file name
    // ("report.pdf") only shows in a normal browser; naming has unit tests.
    ok(shown.payload.title && !shown.payload.title.includes('/'), 'the download appears, with a file name only (no folder)');
    const finished = await waitFor('download finished', events =>
        events.find(e => e.id === shown.id && (e.state === 'ended' || e.state === 'failed')));
    ok(finished.state === 'ended', 'the download finishes as Done');

    // ---- media tabs ----------------------------------------------------
    const youtube = await context.newPage();
    await youtube.goto('https://www.youtube.com/watch?v=midnight');
    const tune = await waitFor('YouTube tab', events => events.find(e =>
        e.id.startsWith('browser:media:chrome:') && e.source.app === 'YouTube' && e.payload.playing));
    ok(tune.payload.title === 'Midnight City' && tune.payload.artist === 'M83', 'a YouTube tab shows its song and artist');
    ok(tune.actions.map(a => a.id).join() === 'play_pause,next', 'Play/Pause and Next (no Previous on this page)');
    await waitFor('YouTube logo', events => events.find(e => e.id === tune.id && e.source.icon?.startsWith('data:image/png;base64,')));
    ok(true, 'the YouTube tab carries its logo');

    invoke(tune.id, 'play_pause');
    await waitFor('paused', events => events.find(e => e.id === tune.id && e.payload.playing === false));
    await sleep(2500);
    ok(list().find(e => e.id === tune.id)?.state !== 'ended', 'a paused tab stays (it does not end by itself)');
    ok(await youtube.evaluate(() => document.querySelector('audio').paused), 'Pause in the island paused the page');
    invoke(tune.id, 'play_pause');
    await waitFor('playing again', events => events.find(e => e.id === tune.id && e.payload.playing === true));
    ok(true, 'Play resumes it');

    await youtube.evaluate(() => {
        document.querySelector('.html5-video-player').classList.add('ad-showing');
        navigator.mediaSession.metadata = new MediaMetadata({title: 'BUY_SHOES_AD_30S_V2', artist: 'Advertiser'});
    });
    const during = await waitFor('ad', events => events.find(e => e.id === tune.id && !('position_ms' in e.payload)));
    ok(during.payload.title === 'Midnight City', 'during an ad the song title stays (ad name hidden)');
    await youtube.evaluate(() => {
        document.querySelector('.html5-video-player').classList.remove('ad-showing');
        navigator.mediaSession.metadata = new MediaMetadata({title: 'Midnight City', artist: 'M83'});
    });

    invoke(tune.id, 'next');
    await sleep(800);
    ok(await youtube.evaluate(() => document.body.dataset.next === 'pressed'), 'Next in the island pressed the page\'s Next button');

    const soundcloud = await context.newPage();
    await soundcloud.goto('https://soundcloud.com/artist/track');
    await waitFor('two tabs', events => events.filter(e => e.id.startsWith('browser:media:chrome:') && e.state !== 'ended').length === 2);
    const sc = list().find(e => e.source.app === 'SoundCloud');
    ok(sc.payload.title === 'Night Drive' && sc.payload.position_ms === 83_000 && sc.payload.duration_ms === 215_000,
        'a SoundCloud tab plays at the same time, with its time read from the page');

    await youtube.close();
    await waitFor('YouTube tab ended', events => events.find(e => e.id === tune.id && e.state === 'ended'));
    ok(true, 'closing a tab ends its media');

    // ---- a call on any site ----------------------------------------------
    const p2p = await context.newPage();
    await p2p.goto('https://p2p.example/share');
    const huddle = await context.newPage();
    await huddle.goto('https://app.slack.com/huddle/T1/C2');
    const webCall = await waitFor('Slack call', events => events.find(e =>
        e.id.startsWith('browser:call:') && e.state !== 'ended'));
    ok(webCall.payload.title === 'Slack' && webCall.payload.confidence === 'confirmed',
        'a call on Slack in the browser shows as a green "Slack" call');
    ok(webCall.actions.length === 0, 'no buttons: mute and leave in the call itself');
    ok(list().filter(e => e.id.startsWith('browser:call:') && e.state !== 'ended').length === 1,
        'a page with a connection but no microphone is not a call');

    const micHint = await waitFor('Slack holds the microphone', events => events.find(e =>
        e.id.startsWith('browser:mic:') && e.state !== 'ended'));
    ok(micHint.source.app === 'Slack', 'the island learns which site holds the microphone');

    await huddle.evaluate(() => window.hangUp());
    await waitFor('call ended', events => events.find(e => e.id === webCall.id && e.state === 'ended'));
    ok(true, 'hanging up ends the call');
    await waitFor('microphone released', events => events.find(e => e.id === micHint.id && e.state === 'ended'));
    ok(true, 'stopping the microphone clears the site');

    // ---- a site that reports its call controls (Media Session) ------------
    const teams = await context.newPage();
    await teams.goto('https://teams.microsoft.com/call');
    const teamsCall = await waitFor('Teams call with controls', events => events.find(e =>
        e.id.startsWith('browser:call:') && e.state !== 'ended' && e.payload.title === 'Microsoft Teams' &&
        e.actions.length > 0));
    ok(teamsCall.payload.muted === false && teamsCall.actions.map(a => a.id).join() === 'mute,leave',
        'a site that reports its microphone gets Mute and Leave');
    invoke(teamsCall.id, 'mute');
    await waitFor('Teams muted', events => events.find(e => e.id === teamsCall.id && e.payload.muted === true));
    ok(await teams.evaluate(() => window.pageMuted) === true, 'Mute in the island used the site\'s own mute');
    invoke(teamsCall.id, 'unmute');
    await waitFor('Teams unmuted', events => events.find(e => e.id === teamsCall.id && e.payload.muted === false));
    ok(await teams.evaluate(() => window.pageMuted) === false, 'Unmute works the same way');
    invoke(teamsCall.id, 'leave');
    await waitFor('Teams left', events => events.find(e => e.id === teamsCall.id && e.state === 'ended'));
    ok(await teams.evaluate(() => window.leftFromIsland) === true, 'Leave used the site\'s own hang up');
} finally {
    await context.close();
}
