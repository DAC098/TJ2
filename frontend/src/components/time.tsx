interface ViewDateProps {
    date: Date
}

function pad_num(value: number) {
    return value.toString(10).padStart(2, '0');
}

const millisecond = 1;
const second = millisecond * 1000;
const minute = second * 60;
const hour = minute * 60;
const day = hour * 24;

export const ViewDate = ({date}: ViewDateProps) => {
    let now = new Date();
    let milli = Math.abs(now.getTime() - date.getTime());
    let days = Math.floor(milli / day);

    if (days > 0) {
        return <span title={date.toISOString()}>
            {date.toDateString()}
        </span>
    } else {
        return <span title={date.toISOString()}>
            {date.toTimeString()}
        </span>
    }
};
