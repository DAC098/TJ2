import { useEffect, useRef } from "react"

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
