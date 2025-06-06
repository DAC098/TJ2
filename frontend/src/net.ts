import MIMEType from "whatwg-mimetype";

export async function res_as_json<T>(res: Response): Promise<T> {
    let content_type = res.headers.get("content-type");

    if (content_type == null) {
        throw new Error("unspecified content-type from response");
    }

    let mime = new MIMEType(content_type);

    console.log(mime);

    if (mime.type !== "application" && mime.subtype !== "json") {
        throw new Error("non json content-type");
    }

    return await res.json();
}

type RequestMethod = "GET" | "HEAD" | "POST" | "PUT" | "DELETE" | "CONNECT" | "OPTIONS" | "TRACE" | "PATCH";

interface RequestOptions {
    headers?: {[header: string]: string}
}

export async function send_json(
    method: RequestMethod,
    url: string,
    data: any,
    {headers = {}}: RequestOptions = {}
): Promise<Response> {
    let body = JSON.stringify(data);

    return await fetch(url, {
        method,
        headers: {
            "content-type": "application/json",
            "content-length": body.length.toString(10),
            ...headers
        },
        body
    });
}
