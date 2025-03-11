interface MimeType {
    type: string,
    subtype: string,
    params: string | null
}

export function parse_mime(given: string): MimeType | null {
    let mime_split = given.split('/');
    let mime_type = mime_split[0];

    if (mime_split.length != 2) {
        return null;
    }

    let mime_subtype = mime_split[1].split(';');

    if (mime_subtype.length > 1) {
        return {
            type: mime_type,
            subtype: mime_subtype[0],
            params: mime_subtype[1]
        };
    } else {
        return {
            type: mime_type,
            subtype: mime_subtype[0],
            params: null
        };
    }
}
