import { year, month, week, day, hour, minute, second, millisecond } from "@/duration";
import { parse } from "date-fns";

const diff_names = ["years", "months", "weeks", "days", "hours", "minutes", "seconds", "milliseconds"];
const diff_names_short = ["y", "m", "w", "d", "h", "m", "s", "ms"];
const diff_order = [year, month, week, day, hour, minute, second, millisecond];

export function time_to_string(time: number, show_milli: boolean = true, short_hand: boolean = false): string {
    let working = time;
    let results = [];

    for (let i = 0; i < diff_order.length; ++i) {
        // critical section
        let value = Math.floor(working / diff_order[i]);
        working %= diff_order[i];

        results.push(value);
    }

    let str_list = [];

    for (let i = 0; i < results.length; ++i) {
        if (!show_milli && i === results.length - 1) {
            continue;
        }

        if (results[i] != 0) {
            str_list.push(`${results[i]} ${short_hand ? diff_names_short[i] : diff_names[i]}`);
        }
    }

    return str_list.join(" ");
}

/**
 * takes the difference between two dates and will display then as
 * years months days hours minutes seonds
 * @param lhs left hand side of operation
 * @param rhs right hand side of operation
 * @returns
 */
export function diff_dates(lhs: Date, rhs: Date, show_milli: boolean = true, short_hand: boolean = false): string {
    // get the timestamps of both dates in milliseconds
    let diff = lhs.getTime() - rhs.getTime();

    return time_to_string(diff, show_milli, short_hand);
}

export function date_to_naive_date(given: Date): string {
    let year = given.getFullYear();
    let month = (given.getMonth() + 1).toString(10).padStart(2, '0');
    let day = given.getDate().toString(10).padStart(2, '0');

    return `${year}-${month}-${day}`;
}

export function naive_date_to_date(given: string): Date | null {
    let rtn = parse(given, "yyyy-MM-dd", new Date());

    if (isNaN(rtn.getTime())) {
        return null;
    } else {
        return rtn;
    }
}

export function same_date(given: Date, ref: Date) {
    return given.getDate() === ref.getDate() &&
        given.getMonth() === ref.getMonth() &&
        given.getFullYear() === ref.getFullYear();
}

export const MONTH_NAMES = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

export const DAY_NAMES = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

export const MILLISECOND = 1;
export const SECOND = MILLISECOND * 1000;
export const MINUTE = SECOND * 60;
export const HOUR = MINUTE * 60;
export const DAY = HOUR * 24;
export const WEEK = DAY * 7;