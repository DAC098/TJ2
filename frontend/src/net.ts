export async function res_as_json<T>(res: Response): Promise<T> {
    let content_type = res.headers.get("content-type");

    if (content_type == null || content_type !== "application/json") {
        throw new Error("unspecified content-type from response");
    }

    if (content_type !== "application/json") {
        throw new Error("non json content-type");
    }

    return await res.json();
}
