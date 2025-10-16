import { clsx, ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs))
}

export function prob_bool(given: number): boolean {
    let calc = Math.floor(Math.random() * 100);

    return calc < given;
}

export async function send_to_clipboard(text: string): Promise<void> {
    let clipboard_data = { "text/plain": text };

    let clipboard_item = new ClipboardItem(clipboard_data);

    await navigator.clipboard.write([clipboard_item]);
}

export function merge_sorted<T>(list_a: T[], list_b: T[], sorter: (a: T, b: T) => boolean): T[] {
    let rtn: T[] = [];
    let a_index = 0;
    let b_index = 0;

    while (a_index < list_a.length && b_index < list_b.length) {
        if (sorter(list_a[a_index], list_b[b_index])) {
            rtn.push(list_a[a_index]);
            a_index += 1;
        } else {
            rtn.push(list_b[b_index]);
            b_index += 1;
        }
    }

    if (a_index < list_a.length) {
        rtn = rtn.concat(list_a);
    }

    if (b_index < list_b.length) {
        rtn = rtn.concat(list_b);
    }

    return rtn;
}