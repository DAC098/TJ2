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
