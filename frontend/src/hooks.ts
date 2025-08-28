import { useState, useEffect } from "react";

export function useObjectUrl(src: string | File | Blob): string | null {
    let [url, set_url] = useState<string | null>(typeof src === "string" ? src : null);

    useEffect(() => {
        if (typeof src === "string") {
            set_url(src);
        } else {
            let obj_url = URL.createObjectURL(src);

            set_url(obj_url);

            return () => {
                URL.revokeObjectURL(obj_url);
            };
        }
    }, [src]);

    return url;
}
