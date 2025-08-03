import * as React from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { DayPicker } from "react-day-picker";

import { cn } from "@/utils";
import { Button, buttonVariants } from "@/components/ui/button";
import { ScrollArea, ScrollBar } from "@/components/ui/scroll-area";

export type CalendarProps = React.ComponentProps<typeof DayPicker>;

/*
 * NOTE: this currently relies on react-day-picker@8.10.x and date-fns@3
 * otherwise the styling is not what it is supposed to be
 */

export function Calendar({
    className,
    classNames,
    showOutsideDays = true,
    ...props
}: CalendarProps) {
    return <DayPicker
        showOutsideDays={showOutsideDays}
        className={cn("p-3", className)}
        classNames={{
            months: "flex flex-col sm:flex-row space-y-4 sm:space-x-4 sm:space-y-0",
            month: "space-y-4",
            caption: "flex justify-center pt-1 relative items-center",
            caption_label: "text-sm font-medium",
            nav: "space-x-1 flex items-center",
            nav_button: cn(
                buttonVariants({ variant: "outline" }),
                "h-7 w-7 bg-transparent p-0 opacity-50 hover:opacity-100"
            ),
            nav_button_previous: "absolute left-1",
            nav_button_next: "absolute right-1",
            table: "w-full border-collapse space-y-1",
            head_row: "flex",
            head_cell:"text-muted-foreground rounded-md w-9 font-normal text-[0.8rem]",
            row: "flex w-full mt-2",
            cell: "h-9 w-9 text-center text-sm p-0 relative [&:has([aria-selected].day-range-end)]:rounded-r-md [&:has([aria-selected].day-outside)]:bg-accent/50 [&:has([aria-selected])]:bg-accent first:[&:has([aria-selected])]:rounded-l-md last:[&:has([aria-selected])]:rounded-r-md focus-within:relative focus-within:z-20",
            day: cn(
                buttonVariants({ variant: "ghost" }),
                "h-9 w-9 p-0 font-normal aria-selected:opacity-100"
            ),
            day_range_end: "day-range-end",
            day_selected:"bg-primary text-primary-foreground hover:bg-primary hover:text-primary-foreground focus:bg-primary focus:text-primary-foreground",
            day_today: "bg-accent text-accent-foreground",
            day_outside: "day-outside text-muted-foreground aria-selected:bg-accent/50 aria-selected:text-muted-foreground",
            day_disabled: "text-muted-foreground opacity-50",
            day_range_middle:"aria-selected:bg-accent aria-selected:text-accent-foreground",
            day_hidden: "invisible",
            //...classNames,
        }}
        components={{
            IconLeft: ({ ...props }) => <ChevronLeft className="h-4 w-4" />,
            IconRight: ({ ...props }) => <ChevronRight className="h-4 w-4" />,
        }}
        {...props}
    />;
}
Calendar.displayName = "Calendar";

const hours_list = [
                      23,22,21,20,
    19,18,17,16,15,14,13,12,11,10,
     9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
];
const minutes_list = [
    59,58,57,56,55,54,53,52,51,50,
    49,48,47,46,45,44,43,42,41,40,
    39,38,37,36,35,34,33,32,31,30,
    29,28,27,26,25,24,23,22,21,20,
    19,18,17,16,15,14,13,12,11,10,
     9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
];
const seconds_list = [
    59,58,57,56,55,54,53,52,51,50,
    49,48,47,46,45,44,43,42,41,40,
    39,38,37,36,35,34,33,32,31,30,
    29,28,27,26,25,24,23,22,21,20,
    19,18,17,16,15,14,13,12,11,10,
     9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
];

interface TimePickerProps {
    value: Date,
    on_change?: (value: Date) => void,
}

export function TimePicker({
    value,
    on_change = (v) => {}
}: TimePickerProps) {
    return <div className="flex flex-col sm:flex-row sm:h-[300px] divide-y sm:divide-y-0 sm:divide-x">
        <ScrollArea className="w-64 sm:w-auto">
            <div className="flex sm:flex-col p-2">
                {hours_list.map((hour) => {
                    return <Button
                        key={hour}
                        size="icon"
                        variant={value.getHours() === hour ? "default" : "ghost"}
                        className="sm:w-full shrink-0 aspect-square"
                        onClick={() => {
                            let new_value = new Date(value);

                            new_value.setHours(hour);

                            on_change(new_value);
                        }}
                    >
                        {hour}
                    </Button>
                })}
            </div>
            <ScrollBar
                orientation="horizontal"
                className="sm:hidden"
            />
        </ScrollArea>
        <ScrollArea className="w-64 sm:w-auto">
            <div className="flex sm:flex-col p-2">
                {minutes_list.map((minute) => {
                    return <Button
                        key={minute}
                        size="icon"
                        variant={value.getMinutes() === minute? "default" : "ghost"}
                        className="sm:w-full shrink-0 aspect-square"
                        onClick={() => {
                            let new_value = new Date(value);

                            new_value.setMinutes(minute);

                            on_change(new_value);
                        }}
                    >
                        {minute}
                    </Button>
                })}
            </div>
            <ScrollBar
                orientation="horizontal"
                className="sm:hidden"
            />
        </ScrollArea>
        <ScrollArea className="w-64 sm:w-auto">
            <div className="flex sm:flex-col p-2">
                {seconds_list.map((second) => {
                    return <Button
                        key={second}
                        size="icon"
                        variant={value.getSeconds() === second? "default" : "ghost"}
                        className="sm:w-full shrink-0 aspect-square"
                        onClick={() => {
                            let new_value = new Date(value);

                            new_value.setSeconds(second);

                            on_change(new_value);
                        }}
                    >
                        {second}
                    </Button>
                })}
            </div>
            <ScrollBar
                orientation="horizontal"
                className="sm:hidden"
            />
        </ScrollArea>
    </div>
}
