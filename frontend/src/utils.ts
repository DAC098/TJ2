import { clsx, ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs))
}

export function prob_bool(given: number): boolean {
    let calc = Math.floor(Math.random() * 100);

    return calc < given;
}
