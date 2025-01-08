import { res_as_json, send_json } from "@/net";

export namespace custom_field {
    export enum TypeName {
        Integer = "Integer",
        IntegerRange = "IntegerRange",
        Float = "Float",
        FloatRange = "FloatRange",
        Time = "Time",
        TimeRange = "TimeRange",
    }

    export interface IntegerType {
        type: TypeName.Integer,
        minimum: number | null,
        maximum: number | null,
    }

    export interface IntegerValue {
        type: TypeName.Integer,
        value: number
    }

    export interface IntegerRangeType {
        type: TypeName.IntegerRange,
        minimum: number | null,
        maximum: number | null,
    }

    export interface IntegerRangeValue {
        type: TypeName.IntegerRange,
        low: number,
        high: number,
    }

    export interface FloatType {
        type: TypeName.Float,
        minimum: number | null,
        maximum: number | null,
        step: number,
        precision: number,
    }

    export interface FloatValue {
        type: TypeName.Float,
        value: number,
    }

    export interface FloatRangeType {
        type: TypeName.FloatRange,
        minimum: number | null,
        maximum: number | null,
        step: number,
        precision: number,
    }

    export interface FloatRangeValue {
        type: TypeName.FloatRange,
        low: number,
        high: number,
    }

    export interface TimeType {
        type: TypeName.Time,
    }

    export interface TimeValue {
        type: TypeName.Time,
        value: string,
    }

    export interface TimeRangeType {
        type: TypeName.TimeRange,
        show_diff: boolean,
    }

    export interface TimeRangeValue {
        type: TypeName.TimeRange,
        low: string,
        high: string,
    }

    export type Type =
        IntegerType |
        IntegerRangeType |
        FloatType |
        FloatRangeType |
        TimeType |
        TimeRangeType;

    export type Value =
        IntegerValue |
        IntegerRangeValue |
        FloatValue |
        FloatRangeValue |
        TimeValue |
        TimeRangeValue;

    export function make_type(given: TypeName): Type {
        switch (given) {
        case TypeName.Integer:
            return {
                type: TypeName.Integer,
                minimum: null,
                maximum: null,
            };
        case TypeName.IntegerRange:
            return {
                type: TypeName.IntegerRange,
                minimum: null,
                maximum: null,
            };
        case TypeName.Float:
            return {
                type: TypeName.Float,
                minimum: null,
                maximum: null,
                step: 0.01,
                precision: 2,
            };
        case TypeName.FloatRange:
            return {
                type: TypeName.FloatRange,
                minimum: null,
                maximum: null,
                step: 0.01,
                precision: 2,
            };
        case TypeName.Time:
            return {
                type: TypeName.Time,
            };
        case TypeName.TimeRange:
            return {
                type: TypeName.TimeRange,
                show_diff: false
            };
        default:
            throw new Error("unknown type name given");
        }
    }
}

export interface JournalPartial {
    id: number,
    uid: string,
    users_id: number,
    name: string,
    description: string | null,
    created: string,
    updated: string | null
}

export interface JournalCustomField {
    id: number,
    uid: string,
    name: string,
    order: number,
    config: custom_field.Type,
    description: string | null,
    created: string,
    updated: string | null,
}

export interface JournalFull {
    id: number,
    uid: string,
    users_id: number,
    name: string,
    description: string | null,
    created: string,
    updated: string | null,
    custom_fields: JournalCustomField[],
}

export interface EntryPartial {
    id: number,
    date: string,
    created: string,
    updated: string | null,
    tags: EntryTagsPartial
}

export interface EntryTagsPartial {
    [key: string]: string | null
}

export interface EntryCustomField {
    custom_fields_id: number,
    value: custom_field.Value,
    created: string,
    updated: string | null,
}

export interface Entry {
    id: number,
    uid: string,
    journals_id: number,
    users_id: number,
    date: string,
    title: string | null,
    contents: string | null,
    created: string,
    updated: string | null,
    tags: EntryTag[],
    files: EntryFile[],
    custom_fields: EntryCustomField[],
}

export interface EntryTag {
    key: string,
    value: string | null,
    created: string,
    updated: string | null,
}

export interface EntryFile {
    id: number,
    uid: string,
    entries_id: number,
    name: string | null,
    mime_type: string,
    mime_subtype: string,
    mime_param: string | null,
    size: number,
    created: string,
    updated: string | null,
    attached?: ClientData,
}

export interface ClientData {
    key: string
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
    mime_param: string | null
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

function pad_num(num: number): string {
    if (num < 10) {
        return "0" + num.toString(10);
    } else {
        return num.toString(10);
    }
}

export function timestamp_name() {
    let now = new Date();
    let month = pad_num(now.getMonth() + 1);
    let day = pad_num(now.getDate());
    let hour = pad_num(now.getHours());
    let minute = pad_num(now.getMinutes());
    let second = pad_num(now.getSeconds());

    return `${now.getFullYear()}-${month}-${day}_${hour}-${minute}-${second}`;
}

export function blank_form(): EntryForm {
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

interface ParsedDate {
    year: number,
    month: number,
    date: number,
}

export function parse_date(given: string): ParsedDate {
    let split = given.split("-");

    return {
        year: parseInt(split[0], 10),
        month: parseInt(split[1], 10),
        date: parseInt(split[2], 10),
    };
}

export function entry_to_form(entry: Entry): EntryForm {
    let tags = [];
    let files = [];
    let custom_fields = [];

    for (let tag of entry.tags) {
        tags.push({
            key: tag.key,
            value: tag.value ?? ""
        });
    }

    for (let file of entry.files) {
        files.push({
            type: "server",
            _id: file.id,
            uid: file.uid,
            name: file.name,
            mime_type: file.mime_type,
            mime_subtype: file.mime_subtype,
            mime_param: file.mime_param,
            created: file.created,
            updated: file.updated,
        });
    }

    for (let field of entry.custom_fields) {
        custom_fields.push({
            custom_fields_id: field.custom_fields_id,
            value: field.value,
        });
    }

    let date = new Date();
    let parsed = parse_date(entry.date);
    date.setFullYear(parsed.year);
    date.setMonth(parsed.month - 1);
    date.setDate(parsed.date);

    return {
        id: entry.id,
        uid: entry.uid,
        date,
        title: entry.title ?? "",
        contents: entry.contents ?? "",
        tags,
        files,
        custom_fields,
    };
}

export function get_date(date: Date): string {
    let month = (date.getMonth() + 1)
        .toString(10)
        .padStart(2, '0');
    let day = date.getDate()
        .toString(10)
        .padStart(2, '0');

    return `${date.getFullYear()}-${month}-${day}`;
}

export async function get_journals() {
    let res = await fetch("/journals");

    if (res.status !== 200) {
        return null;
    }

    return await res.json() as JournalPartial[];
}

export async function get_journal(journals_id: string) {
    let res = await fetch(`/journals/${journals_id}`);

    if (res.status !== 200) {
        return null;
    }

    return await res.json() as JournalFull;
}

export async function retrieve_entry(
    journals_id: string,
    entries_id: string,
): Promise<Entry | null> {
    let res = await fetch(`/journals/${journals_id}/entries/${entries_id}`);

    if (res.status === 404) {
        return null;
    }

    if (res.status !== 200) {
        throw new Error("non 200 response status");
    }

    return await res_as_json<Entry>(res);
}

export async function create_entry(
    journals_id: string,
    entry: EntryForm,
) {
    let sending = {
        date: get_date(entry.date),
        title: entry.title,
        contents: entry.contents,
        tags: entry.tags,
        files: [],
    };

    for (let file of entry.files) {
        if (file.type == "server") {
            sending.files.push({
                id: file._id,
                name: file.name,
            });
        } else {
            sending.files.push({
                key: file.key,
                name: file.name,
            });
        }
    }

    let res = await send_json(
        "POST",
        `/journals/${journals_id}/entries`,
        sending
    );

    if (res.status !== 201) {
        throw new Error("failed to create new entry");
    }

    return await res_as_json<Entry>(res);
}

export async function update_entry(
    journals_id: string,
    entries_id: string,
    entry: EntryForm,
) {
    let sending = {
        date: get_date(entry.date),
        title: entry.title,
        contents: entry.contents,
        tags: entry.tags,
        files: [],
    };

    for (let file of entry.files) {
        if (file.type == "server") {
            sending.files.push({
                id: file._id,
                name: file.name,
            });
        } else {
            sending.files.push({
                key: file.key,
                name: file.name,
            });
        }
    }

    let res = await send_json(
        "PATCH",
        `/journals/${journals_id}/entries/${entries_id}`,
        sending
    );

    if (res.status !== 200) {
        throw new Error("failed to update entry");
    }

    return await res_as_json<Entry>(res);
}

export async function delete_entry(
    journals_id: string,
    entries_id: string,
) {
    let res = await fetch(`/journals/${journals_id}/entries/${entries_id}`, {
        method: "DELETE"
    });

    if (res.status !== 200) {
        throw new Error("failed to delete entry");
    }
}

export async function upload_data(
    journals_id: number,
    entries_id: number,
    file_entry: EntryFile,
    ref: LocalFile | InMemoryFile | ServerFile,
): Promise<boolean> {
    try {
        let path = `/journals/${journals_id}/entries/${entries_id}/${file_entry.id}`;

        console.log(ref);

        switch (ref.type) {
        case "in-memory":
        case "local":
            let result = await fetch(path, {
                method: "PUT",
                headers: {
                    "content-type": ref.data.type,
                    "content-length": ref.data.size.toString(10),
                },
                body: ref.data
            });

            if (result.status !== 200) {
                return false;
            } else {
                return true;
            }
        case "server":
            return true;
        }
    } catch(err) {
        console.log("failed to upload data", err);

        return false;
    }
}
