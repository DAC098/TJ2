import { res_as_json } from "./net";

export interface JournalEntry {
    id: number,
    uid: string,
    journals_id: number,
    users_id: number,
    date: string,
    title: string | null,
    contents: string | null,
    created: string,
    updated: string | null,
    tags: JournalTag[],
    files: JournalFile[],
}

export interface JournalTag {
    key: string,
    value: string | null,
    created: string,
    updated: string | null,
}

export interface JournalFile {
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
    attached?: JournalClientData,
}

export interface JournalClientData {
    key: string
}

export interface EntryTagForm {
    key: string,
    value: string,
}

export interface InMemoryFile {
    type: "in-memory",
    key: string,
    src: URL,
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
    src: URL,
    name: string,
    data: File,
}

export type EntryFileForm =
    InMemoryFile |
    ServerFile;

export interface EntryForm {
    date: string,
    title: string,
    contents: string,
    tags: EntryTagForm[],
    files: EntryFileForm[],
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
        date: get_date(today),
        title: "",
        contents: "",
        tags: [],
        files: [],
    };
}

export function entry_to_form(entry: JournalEntry): EntryForm {
    let tags = [];
    let files = [];

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

    return {
        date: entry.date,
        title: entry.title ?? "",
        contents: entry.contents ?? "",
        tags,
        files,
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

export async function retrieve_entry(date: string): Promise<JournalEntry | null> {
    let res = await fetch(`/entries/${date}`);

    if (res.status === 404) {
        return null;
    }

    if (res.status !== 200) {
        throw new Error("non 200 response status");
    }

    return await res_as_json<JournalEntry>(res);
}

export async function create_entry(entry: EntryForm) {
    let sending = {
        date: entry.date,
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

    let body = JSON.stringify(sending);
    let res = await fetch("/entries", {
        method: "POST",
        headers: {
            "content-type": "application/json",
            "content-length": body.length.toString(10),
        },
        body: body
    });

    if (res.status !== 201) {
        throw new Error("failed to create new entry");
    }

    return await res_as_json<JournalEntry>(res);
}

export async function update_entry(date: string, entry: EntryForm) {
    let body = JSON.stringify(entry);
    let res = await fetch(`/entries/${date}`, {
        method: "PATCH",
        headers: {
            "content-type": "application/json",
            "content-length": body.length.toString(10),
        },
        body: body
    });

    if (res.status !== 200) {
        throw new Error("failed to update entry");
    }

    return await res_as_json<JournalEntry>(res);
}

export async function delete_entry(date: string) {
    let res = await fetch(`/entries/${date}`, {
        method: "DELETE"
    });

    if (res.status !== 200) {
        throw new Error("failed to delete entry");
    }
}

export async function upload_data(
    date: string,
    file_entry: JournalFile,
    ref: LocalFile | InMemoryFile
): Promise<boolean> {
    try {
        let path = `/entries/${date}/${file_entry.id}`;

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
        }
    } catch(err) {
        console.log("failed to upload data", err);

        return false;
    }
}
