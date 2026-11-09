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

const MEET = `<!doctype html><html><head><title>Meet - Weekly sync</title></head><body>
<button id="mic" aria-label="Turn off microphone" data-is-muted="false"><i class="google-material-icons">mic</i></button>
<button id="cam" aria-label="Turn off camera" data-is-muted="false"><i class="google-material-icons">videocam</i></button>
<button id="leave" aria-label="Leave call"><i class="google-material-icons">call_end</i></button>
<script>
const mic = document.getElementById('mic');
mic.onclick = () => {
  const muted = mic.dataset.isMuted !== 'true';
  mic.dataset.isMuted = String(muted);
  mic.querySelector('i').textContent = muted ? 'mic_off' : 'mic';
};
document.getElementById('leave').onclick = () => { document.body.innerHTML = '<p>You left the meeting</p>'; };
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

const context = await chromium.launchPersistentContext(profile, {
    headless: false,
    acceptDownloads: true,
    args: [`--disable-extensions-except=${EXTENSION}`, `--load-extension=${EXTENSION}`],
});
try {
    await context.route('https://meet.google.com/**', route =>
        route.fulfill({status: 200, contentType: 'text/html', body: MEET}));
    await context.route('https://files.example/**', route =>
        route.fulfill({status: 200, body: Buffer.alloc(256 * 1024),
            headers: {'content-type': 'application/octet-stream', 'content-disposition': 'attachment; filename="report.pdf"'}}));

    const page = await context.newPage();
    await page.goto('https://meet.google.com/abc-defg-hij');

    const call = await waitFor('Meet call', events => events.find(e => e.id.startsWith('browser:meet:') && e.state !== 'ended'));
    ok(call.payload.title === 'Weekly sync', 'the call appears with the meeting name');
    ok(call.payload.confidence === 'confirmed' && call.payload.muted === false, 'confirmed call, microphone on');
    ok(call.actions.map(a => a.id).join() === 'mute,leave', 'Mute and Leave buttons');

    invoke(call.id, 'mute');
    await waitFor('muted', events => events.find(e => e.id === call.id && e.payload.muted === true));
    ok(await page.getAttribute('#mic', 'data-is-muted') === 'true', 'Mute in the island pressed Meet\'s own microphone button');

    invoke(call.id, 'unmute');
    await waitFor('unmuted', events => events.find(e => e.id === call.id && e.payload.muted === false));
    ok(true, 'Unmute works the same way');

    invoke(call.id, 'leave');
    await waitFor('call ended', events => events.find(e => e.id === call.id && e.state === 'ended'));
    ok(await page.locator('#leave').count() === 0, 'Leave pressed Meet\'s leave button and the call ended');

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
} finally {
    await context.close();
}
