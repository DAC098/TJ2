import { time_to_string } from "@/time";
import { addDays, addHours, addMinutes, addSeconds } from "date-fns";
import { useEffect, useRef, useState } from "react"

export function useTimer(callback: () => void): [(ms: number) => void, () => void] {
    let timer_ref = useRef<number>();
    let cb_ref = useRef(callback);

    useEffect(() => {
        cb_ref.current = callback;
    }, [callback]);

    return [
        (ms: number) => {
            if (timer_ref.current) {
                window.clearTimeout(timer_ref.current);
            }

            timer_ref.current = window.setTimeout(() => {
                window.clearTimeout(timer_ref.current);
                cb_ref.current();
            }, ms);
        },
        () => {
            if (timer_ref.current) {
                window.clearTimeout(timer_ref.current);
            }
        }
    ];
}

export function useInterval(callback: () => void, time: number) {
    let timer_ref = useRef<number>();
    let interval_ref = useRef<number>(time);
    let cb_ref = useRef(callback);

    useEffect(() => {
        cb_ref.current = callback;
    }, [callback]);

    useEffect(() => {
        interval_ref.current = time;

        if (timer_ref.current)
            window.clearInterval(timer_ref.current);

        if (time <= 0)
            return;

        timer_ref.current = window.setInterval(() => {
            cb_ref.current();
        }, interval_ref.current);
    }, [time]);
}

export enum DateInterval {
    Second = 0,
    Minute = 1,
    Hour = 2,
    Day = 3,
}

function get_next(ref: Date, interval: DateInterval) {
    let next = new Date(ref);
    next.setMilliseconds(0);

    switch (interval) {
        case DateInterval.Second:
            return addSeconds(next, 1);
        case DateInterval.Minute:
            next.setSeconds(0);

            return addMinutes(next, 1);
        case DateInterval.Hour:
            next.setSeconds(0);
            next.setMinutes(0);

            return addHours(next, 1);
        case DateInterval.Day:
            next.setSeconds(0);
            next.setMinutes(0);
            next.setHours(0);

            return addDays(next, 1);
    }
}

function next_timeout(interval: DateInterval): [Date, number] {
    let now = new Date();
    let next = get_next(now, interval);

    return [now, next.getTime() - now.getTime()];
}

export function use_date(interval: DateInterval = DateInterval.Day) {
    let [date, set_date] = useState(new Date());

    useEffect(() => {
        let ref = 0;

        const set_timer = (ms: number) =>   window.setTimeout(() => {
            let [now, ts] = next_timeout(interval);

            set_date(now);

            ref = set_timer(ts);
        }, ms);

        let [now, ts] = next_timeout(interval);

        ref = set_timer(ts);

        return () => window.clearTimeout(ref);
    }, [interval]);

    return date;
}