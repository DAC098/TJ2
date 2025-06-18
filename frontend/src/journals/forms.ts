import {
    custom_field,
} from "@/journals/api";

export interface JournalForm {
    name: string,
    description: string,
    custom_fields: JournalCustomFieldForm[],
    peers: JournalPeerForm[],
}

export interface JournalPeerForm {
    user_peers_id: number,
    name: string,
    synced: string | null,
}

export interface JournalCustomFieldForm {
    _id: number | null,
    uid: string | null,
    name: string,
    order: number,
    config: custom_field.Type,
    description: string,
}

export interface EntryCustomFieldForm {
    custom_fields_id: number,
    value: custom_field.Value,
}

export interface EntryTagForm {
    key: string,
    value: string,
}

export interface InMemoryFile {
    type: "in-memory",
    key: string,
    data: Blob,
    name: string,
    mime_type: string,
    mime_subtype: string,
    mime_param: string | null,
}

export interface ServerFile {
    type: "server",
    _id: number,
    uid: string,
    name: string,
    mime_type: string,
    mime_subtype: string,
    mime_param: string | null,
    created: string,
    updated: string | null
}

export interface LocalFile {
    type: "local",
    key: string,
    name: string,
    data: File,
    mime_type: string,
    mime_subtype: string,
    mime_param: string | null,
}

export type EntryFileForm =
    InMemoryFile |
    ServerFile |
    LocalFile;

export interface EntryForm {
    id: number | null,
    uid: string | null,
    date: Date,
    title: string,
    contents: string,
    tags: EntryTagForm[],
    files: EntryFileForm[],
    custom_fields: EntryCustomFieldForm[],
}

export function blank_entry_form(): EntryForm {
    let today = new Date();

    return {
        id: null,
        uid: null,
        date: today,
        title: "",
        contents: "",
        tags: [],
        files: [],
        custom_fields: [],
    };
}

export function blank_journal_form(): JournalForm {
    return {
        name: "",
        description: "",
        custom_fields: [],
        peers: [],
    };
}

export function blank_journal_custom_field_form(type: custom_field.TypeName): JournalCustomFieldForm {
    return {
        _id: null,
        uid: null,
        name: "",
        order: 0,
        config: custom_field.make_type(type),
        description: "",
    };
}
