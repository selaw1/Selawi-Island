// A short "Copied" bubble when something is copied (Ctrl+C), from GNOME's
// own clipboard. Only the kind of data (text, image, files) is used,
// never the content. Pasting cannot be shown: GNOME does not announce it.

import Meta from 'gi://Meta';

import {clipboardInput} from './shell-model.js';

export class ClipboardSource {
    /** @param {(input: object) => void} submit - sends one event input */
    constructor(submit) {
        this._submit = submit;
        this._selection = global.display.get_selection();
        this._selection.connectObject('owner-changed', (_selection, type, owner) => {
            // GNOME keeps the content when an app closes by becoming the
            // owner itself (a memory source): that is not a new copy.
            if (type !== Meta.SelectionType.SELECTION_CLIPBOARD || !owner ||
                (Meta.SelectionSourceMemory && owner instanceof Meta.SelectionSourceMemory))
                return;
            this._submit({...clipboardInput(owner.get_mimetypes()), state: 'started'});
        }, this);
    }

    destroy() {
        this._selection.disconnectObject(this);
        this._selection = null;
    }
}
