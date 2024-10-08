import { res_as_json } from "./net";

export interface JournalEntry {
    id: number,
    users_id: number,
    date: string,
    title: string | null,
    contents: string | null,
    created: string,
    updated: string,
    tags: JournalTag[],
}

export interface JournalTag {
    key: string,
    value: string | null,
    created: string,
    updated: string | null,
}

export interface EntryTagForm {
    key: string,
    value: string,
}

export interface EntryForm {
    date: string,
    title: string,
    contents: string,
    tags: EntryTagForm[],
}

export function blank_form(): EntryForm {
    let today = new Date();

    return {
        date: get_date(today),
        title: "",
        contents: "",
        tags: [],
    };
}

export function entry_to_form(entry: JournalEntry): EntryForm {
    let tags = [];

    for (let tag of entry.tags) {
        tags.push({
            key: tag.key,
            value: tag.value ?? ""
        });
    }

    return {
        date: entry.date,
        title: entry.title ?? "",
        contents: entry.contents ?? "",
        tags
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
    let body = JSON.stringify(entry);
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

    return await res_as_json<JournalEntry>(res);
}
