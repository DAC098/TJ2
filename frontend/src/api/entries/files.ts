import { res_as_json, ApiError, ErrorJson } from "@/net";

export interface ClientData {
    key: string
}

export interface RequestedFile {
    type: "requested",
    _id: number,
    uid: string,
    name: string,
    attached?: ClientData,
}

export interface ReceivedFile {
    type: "received",
    _id: number,
    uid: string,
    name: string,
    mime_type: string,
    mime_subtype: string,
    mime_param: string | null,
    created: string,
    updated: string | null
}

export interface UploadData {
    journals_id: string | number,
    entries_id: string | number,
    file_entry_id: string | number,
    data: Blob | File,
}

export async function upload_data({
    journals_id,
    entries_id,
    file_entry_id,
    data,
}: UploadData): Promise<ReceivedFile> {
    let path = `/journals/${journals_id}/entries/${entries_id}/${file_entry_id}`;

    let content_type = data.type;

    if (content_type.length === 0) {
        content_type = "application/octet-stream";
    }

    let result = await fetch(path, {
        method: "PUT",
        headers: {
            "content-type": content_type,
            "content-length": data.size.toString(10),
        },
        body: data
    });

    if (result.status !== 200) {
        let {error, message, ...rest} = await res_as_json<ErrorJson>(result);

        throw new ApiError(error, {message, data: rest});
    } else {
        return await res_as_json(result);
    }
}
