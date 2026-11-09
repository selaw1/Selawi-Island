import {test} from 'node:test';
import assert from 'node:assert/strict';

import {parseEventList, validateEvent} from '../../gnome-extension/services/protocol.js';

// Exactly what selawi-islandd sends (copied from a real List reply).
const FROM_DAEMON = '[{"id":"n1","source":{"app":"Test","integration":"smoke"},"category":"notification",' +
    '"state":"started","priority":"notification","persistent":false,"created_at":1794249586017,' +
    '"updated_at":1794249586017,"expires_at":1794249586817,"payload":{"type":"notification","title":"Hello"},' +
    '"actions":[{"id":"open","label":"Open"}]},' +
    '{"id":"c","source":{"app":"Slack","integration":"x"},"category":"call","state":"updated","priority":"call",' +
    '"persistent":true,"created_at":1,"updated_at":2,"payload":{"type":"call","title":"Standup","confidence":"probable"},"actions":[]}]';

test('parses a real daemon list and normalizes optional fields', () => {
    const {events, errors} = parseEventList(FROM_DAEMON);
    assert.deepEqual(errors, []);
    assert.equal(events.length, 2);
    assert.equal(events[0].expires_at, 1794249586817);
    assert.equal(events[1].expires_at, null);
    assert.equal(events[1].group, null);
    assert.equal(events[1].priority, 'call');
});

test('drops invalid entries but keeps valid ones', () => {
    const list = JSON.parse(FROM_DAEMON);
    list.push({...list[0], id: 'bad id'});
    list.push({...list[0], id: 'x', priority: 'urgent'});
    list.push({...list[0], id: 'y', created_at: -1});
    list.push({...list[0], id: 'z', created_at: undefined});
    const {events, errors} = parseEventList(JSON.stringify(list));
    assert.equal(events.length, 2);
    assert.equal(errors.length, 4);
});

test('rejects non-list replies without throwing', () => {
    assert.deepEqual(parseEventList('not json').events, []);
    assert.deepEqual(parseEventList('{"a":1}').events, []);
    assert.equal(validateEvent(null).ok, false);
});
