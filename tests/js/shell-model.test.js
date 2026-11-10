import {test} from 'node:test';
import assert from 'node:assert/strict';

import {clipboardInput, isQuiet, shellNotificationInput} from '../../gnome-extension/services/shell-model.js';
import {validateInput} from '../../gnome-extension/services/protocol.js';
import {describe} from '../../gnome-extension/ui/view-model.js';

const valid = input => {
    const r = validateInput({...input, state: 'started'});
    assert.ok(r.ok, r.error);
};

test('GNOME Shell notifications', () => {
    const shot = shellNotificationInput({app: 'Screenshot', title: 'Screenshot captured',
        body: 'You can paste the image from the clipboard.', urgency: 1, seq: 4, showBody: true});
    valid(shot);
    assert.equal(shot.id, 'notifications:gnome-shell:4', 'dropped by the daemon when Notifications is off');
    assert.equal(shot.source.app, 'Screenshot');
    assert.equal(shot.payload.body, 'You can paste the image from the clipboard.');
    assert.equal(shellNotificationInput({app: 'X', title: 'T', body: 'secret', urgency: 1, seq: 1, showBody: false}).payload.body,
        undefined, 'text hidden when "Show notification text" is off');
    const critical = shellNotificationInput({app: 'Network', title: 'Sign in', urgency: 3, seq: 2, showBody: true});
    assert.equal(critical.priority, 'critical');
    valid(critical);
    assert.equal(shellNotificationInput({app: 'X', title: ' <b></b> ', urgency: 1, seq: 3, showBody: true}), null);
});

test('Copied: the kind of data only, and quiet', () => {
    const text = clipboardInput(['text/plain;charset=utf-8', 'UTF8_STRING']);
    valid(text);
    assert.equal(text.payload.title, 'Copied');
    assert.equal(clipboardInput(['image/png']).payload.title, 'Copied image');
    assert.equal(clipboardInput(['x-special/gnome-copied-files', 'text/uri-list']).payload.title, 'Copied files');
    assert.ok(isQuiet({...text, state: 'started'}), 'does not open the island');
    assert.ok(!isQuiet({source: {integration: 'notifications'}}));
    const vm = describe({...text, state: 'started', created_at: 0, updated_at: 0}, 0);
    assert.equal(vm.iconName, 'edit-copy-symbolic');
    assert.equal(describe({...clipboardInput(['image/png']), state: 'started', created_at: 0, updated_at: 0}, 0).iconName,
        'image-x-generic-symbolic');
});
