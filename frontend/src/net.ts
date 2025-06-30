//@ts-ignore
import MIMEType from "whatwg-mimetype";

interface ErrorJson {
    error: string,
    message?: string,
}

interface ApiErrorOptions {
    message?: string,
    source?: unknown,
    data?: any,
}

export class ApiError extends Error {
    public kind: string;
    public data: any;

    constructor(kind: string, {message, source, data}: ApiErrorOptions = {}) {
        // @ts-ignore
        super(message, {cause: source});

        this.kind = kind;
        this.data = data;
    }
}

export async function res_as_json<T = any>(res: Response): Promise<T> {
    let content_type = res.headers.get("content-type");

    if (content_type == null) {
        throw new ApiError("NoContentType");
    }

    let mime = new MIMEType(content_type);

    if (mime.type !== "application" && mime.subtype !== "json") {
        throw new ApiError("InvalidJSON");
    }

    try {
        return await res.json();
    } catch (err) {
        throw new ApiError("InvalidJSON", {source: err});
    }
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
            ...headers,
            "content-type": "application/json; charset=utf-8",
            "content-length": body.length.toString(10),
        },
        body
    });
}

export async function req_json(
    method: RequestMethod,
    url: string,
    data?: any,
): Promise<Response> {
    let headers = {
        "accept": "application/json; charset=utf-8",
    };

    if (data != null) {
        let body = JSON.stringify(data);

        return await fetch(url, {
            method,
            headers: {
                ...headers,
                "content-type": "application/json; charset=utf-8",
                "content-length": body.length.toString(10),
            },
            body
        });
    } else {
        return await fetch(url, {method, headers});
    }
}

export async function req_api_json<T = any>(
    method: RequestMethod,
    url: string,
    data?: any,
): Promise<T> {
    let response = await req_json(method, url, data);

    if (response.status >= 400) {
        let {error, message, ...rest} = await res_as_json<ErrorJson>(response);

        throw new ApiError(error, {message, data: rest});
    }

    return await res_as_json<T>(response);
}