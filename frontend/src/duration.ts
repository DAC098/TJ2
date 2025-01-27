export const milliseconds_in_second = 1000;
export const seconds_in_minute = 60;
export const minutes_in_hour = 60;
export const hours_in_day = 24;
export const days_in_week = 7;
export const days_in_month = 30;
export const days_in_year = 365;

export const millisecond = 1
export const second = milliseconds_in_second * millisecond;
export const minute = seconds_in_minute * second;
export const hour = minutes_in_hour * minute;
export const day = hours_in_day * hour;
export const week = days_in_week * day;
export const month = days_in_month * day;
export const year = days_in_year * day;

export function durationSeconds(seconds: number) {
    return seconds * second;
}

export function durationMinutes(minutes: number) {
    return minutes * minute;
}

export function durationHours(hours: number) {
    return hours * hour;
}

export function durationDays(days: number) {
    return days * day;
}

export function durationWeek(weeks: number) {
    return weeks * week;
}

export function durationMonth(months: number) {
    return months * month;
}

export function durationYears(years: number) {
    return years * year;
}

export function timestampSeconds(ts: number) {
    return ts / second;
}

export function timestampMinutes(ts: number) {
    return ts / minute;
}

export function timestampHours(ts: number) {
    return ts / hour;
}

export function timestampDays(ts: number) {
    return ts / day;
}

export function timestampWeeks(ts: number) {
    return ts / week;
}

export function timestampMonths(ts: number) {
    return ts / month;
}

export function timestampYears(ts: number) {
    return ts / year;
}
