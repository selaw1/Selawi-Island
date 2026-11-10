import {test} from 'node:test';
import assert from 'node:assert/strict';

import {buildReport, extensionStatus, integrationText, integrationTitle, osName} from '../../gnome-extension/settings/report.js';

test('extension states as GNOME 51 reports them', () => {
    assert.deepEqual(extensionStatus({state: 1}), {text: 'On', ok: true, canTurnOn: false});
    assert.equal(extensionStatus({state: 2}).canTurnOn, true);
    assert.equal(extensionStatus({state: 6}).canTurnOn, true, 'initialized but never enabled');
    assert.equal(extensionStatus({state: 3, error: 'TypeError: x is undefined'}).text, 'Error: TypeError: x is undefined');
    assert.equal(extensionStatus({state: 4}).text, 'Not compatible with this GNOME version');
    assert.equal(extensionStatus({}).ok, false);
    assert.equal(extensionStatus(null).text, 'Not installed, or GNOME Shell is not running');
});

test('report: versions and states only', () => {
    const report = buildReport({
        os: osName('NAME="Arch Linux"\nPRETTY_NAME="Arch Linux"\n'),
        shellVersion: '51.1',
        daemon: {version: '0.1.0', protocol: 1, events: 3},
        extension: {state: 1, version: 1},
        extensionErrors: [],
        integrations: [
            {id: 'network', status: 'active', detail: 'on MyHomeWiFi'},
            {id: 'bluez', status: 'active', detail: 'Galaxy Buds connected'},
            {id: 'health', status: 'disabled'},
        ],
    });
    assert.match(report, /System: Arch Linux/);
    assert.match(report, /GNOME Shell: 51\.1/);
    assert.match(report, /Service: running, version 0\.1\.0, protocol 1, 3 event\(s\)/);
    assert.match(report, /Extension: On \(version 1\)/);
    assert.match(report, /network: Active/);
    assert.match(report, /health: Off/);
    assert.doesNotMatch(report, /MyHomeWiFi|Galaxy/, 'no network or device names');
    assert.match(buildReport({os: 'x', shellVersion: null, daemon: null, extension: null, extensionErrors: [], integrations: []}),
        /Service: not running/);
    assert.equal(integrationText({status: 'active', detail: 'battery at 64%'}), 'Active — battery at 64%');
    assert.equal(integrationTitle({name: 'Media players (MPRIS)'}), 'Media players');
    assert.equal(integrationTitle({name: 'System warnings'}), 'System warnings');
});
