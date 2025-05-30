import { format } from "date-fns";
import { Plus, CalendarIcon } from "lucide-react";
import { useFieldArray, useFormContext } from "react-hook-form";

import { Button } from "@/components/ui/button";
import { Calendar, TimePicker } from "@/components/ui/calendar";
import { Checkbox } from "@/components/ui/checkbox";
import {
    FormControl,
    FormDescription,
    FormField,
    FormItem,
    FormLabel,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import {
    Popover,
    PopoverContent,
    PopoverTrigger,
} from "@/components/ui/popover";
import { ScrollArea, ScrollBar } from "@/components/ui/scroll-area";
import { Switch } from "@/components/ui/switch";
import { EntryForm, JournalCustomField, custom_field } from "@/journals/api";
import { JournalForm } from "@/journals/forms";
import { diff_dates } from "@/time";
import { cn } from "@/utils";

export interface CustomFieldEntryCellProps {
    value: custom_field.Value,
    config: custom_field.Type,
}

export function CustomFieldEntryCell({value: v, config}: CustomFieldEntryCellProps) {
    if (config.type !== v.type) {
        return null;
    }

    switch (config.type) {
        case custom_field.TypeName.Integer: {
            let value = v as custom_field.IntegerValue;

            return <span>{value.value}</span>;
        }
        case custom_field.TypeName.IntegerRange: {
            let value = v as custom_field.IntegerRangeValue;

            return <span>
                {`${value.low} - ${value.high}`}
            </span>;
        }
        case custom_field.TypeName.Float: {
            let value = v as custom_field.FloatValue;

            return <span>{value.value}</span>;
        }
        case custom_field.TypeName.FloatRange: {
            let value = v as custom_field.FloatRangeValue;

            return <span>
                {`${value.low} - ${value.high}`}
            </span>;
        }
        case custom_field.TypeName.TimeRange: {
            let value = v as custom_field.TimeRangeValue;

            let start = new Date(value.low)
            let end = new Date(value.high);

            if (config.show_diff) {
                return <span className="text-nowrap">{diff_dates(end, start, true, true)}</span>;
            } else {
                return <span className="text-nowrap">
                    {`${start} - ${end}`}
                </span>;
            }
        }
        default:
            return null;
    }
}

export interface CustomFieldEntriesProps {
}

export function CustomFieldEntries({}: CustomFieldEntriesProps) {
    const form = useFormContext<EntryForm>();
    const custom_fields = useFieldArray<EntryForm, "custom_fields">({
        control: form.control,
        name: "custom_fields",
    });

    return <div className="space-y-4">
        <div className="flex flex-row flex-nowrap gap-x-4 items-center">
            Custom Fields
        </div>
        <div className="flex flex-row flex-wrap gap-4">
            {custom_fields.fields.map((field, index) => {
                let value_input = null;

                switch (field.type) {
                case custom_field.TypeName.Integer:
                    value_input = <IntegerValue index={index} config={field.config}/>;
                    break;
                case custom_field.TypeName.IntegerRange:
                    value_input = <IntegerRangeValue index={index} config={field.config}/>
                    break;
                case custom_field.TypeName.Float:
                    value_input = <FloatValue index={index} config={field.config}/>;
                    break;
                case custom_field.TypeName.FloatRange:
                    value_input = <FloatRangeValue index={index} config={field.config}/>;
                    break;
                case custom_field.TypeName.Time:
                    break;
                case custom_field.TypeName.TimeRange:
                    value_input = <TimeRangeValue index={index} config={field.config}/>;
                    break;
                }

                return <div key={field.id} className="flex-none w-[calc(50%-1rem)] space-y-2">
                    <FormField control={form.control} name={`custom_fields.${index}.enabled`} render={({field: enbl_field}) => {
                        return <FormItem>
                            <div className="flex flex-row flex-nowrap items-center gap-x-2">
                                <FormControl>
                                    <Checkbox checked={enbl_field.value} onCheckedChange={enbl_field.onChange}/>
                                </FormControl>
                                <FormLabel>{field.name}</FormLabel>
                            </div>
                        </FormItem>;
                    }}/>
                    {value_input}
                </div>;
            })}
        </div>
    </div>;
}

export interface IntegerConfigProps {
    id: string,
    config: custom_field.IntegerType | custom_field.IntegerRangeType,
    index: number,
}

export function IntegerConfig({config, index}:IntegerConfigProps) {
    const form = useFormContext<JournalForm>();

    return <div className="flex flex-nowrap gap-x-4">
        <FormField control={form.control} name={`custom_fields.${index}.config.minimum`} render={({field: min_field}) => {
            return <FormItem className="w-1/2">
                <div className="flex flex-row flex-nowrap items-center gap-x-2">
                    <FormControl>
                        <Checkbox checked={min_field.value != null} onCheckedChange={(v) => {
                            min_field.onChange(v ? 0 : null)
                        }}/>
                    </FormControl>
                    <FormLabel>Minimum</FormLabel>
                </div>
                <FormControl>
                    <Input
                        ref={min_field.ref}
                        disabled={min_field.disabled || min_field.value == null}
                        name={min_field.name}
                        value={min_field.value ?? 0}
                        className="w-1/2"
                        type="number"
                        step="1"
                        onBlur={min_field.onBlur}
                        onChange={ev => {
                            min_field.onChange(parseInt(ev.target.value, 10));
                        }}
                    />
                </FormControl>
                <FormDescription>
                    The minimum value that an integer value can be (inclusive)
                </FormDescription>
            </FormItem>;
        }}/>
        <FormField control={form.control} name={`custom_fields.${index}.config.maximum`} render={({field: max_field}) => {
            return <FormItem className="w-1/2">
                <div className="flex flex-row flex-nowrap items-center gap-x-2">
                    <FormControl>
                        <Checkbox checked={max_field.value != null} onCheckedChange={(v) => {
                            max_field.onChange(v ? 10 : null)
                        }}/>
                    </FormControl>
                    <FormLabel>Maximum</FormLabel>
                </div>
                <FormControl>
                    <Input
                        ref={max_field.ref}
                        disabled={max_field.disabled || max_field.value == null}
                        name={max_field.name}
                        value={max_field.value ?? 0}
                        className="w-1/2"
                        type="number"
                        step="1"
                        onBlur={max_field.onBlur}
                        onChange={ev => {
                            max_field.onChange(parseInt(ev.target.value, 10));
                        }}
                    />
                </FormControl>
                <FormDescription>
                    The maximum value that an integer value can be (inclusive)
                </FormDescription>
            </FormItem>
        }}/>
    </div>;
}

export interface IntegerValueProps {
    index: number,
    config: custom_field.IntegerType,
}

export function IntegerValue({index, config}: IntegerValueProps) {
    const form = useFormContext<EntryForm>();
    const enabled = form.watch(`custom_fields.${index}.enabled`);

    return <div className="flex flow-row flex-nowrap items-center gap-x-4">
        <FormField control={form.control} name={`custom_fields.${index}.value.value`} render={({field: int_field}) => {
            return <FormItem className="w-1/2">
                <FormLabel>Value</FormLabel>
                <FormControl>
                    <Input
                        ref={int_field.ref}
                        name={int_field.name}
                        type="number"
                        min={config.minimum}
                        max={config.maximum}
                        disabled={!enabled || int_field.disabled}
                        value={int_field.value}
                        onBlur={int_field.onBlur}
                        onChange={(ev) => {
                            int_field.onChange(parseInt(ev.target.value, 10));
                        }}
                    />
                </FormControl>
            </FormItem>;
        }}/>
    </div>;
}

export interface IntegerRangeValueProps {
    index: number,
    config: custom_field.IntegerRangeType,
}

export function IntegerRangeValue({index, config}: IntegerRangeValueProps) {
    const form = useFormContext<EntryForm>();
    const enabled = form.watch(`custom_fields.${index}.enabled`);

    return <div className="flex flow-row flex-nowrap items-center gap-x-4">
        <FormField control={form.control} name={`custom_fields.${index}.value.low`} render={({field: low_field}) => {
            return <FormItem className="w-1/2">
                <FormLabel>Low</FormLabel>
                <FormControl>
                    <Input
                        ref={low_field.ref}
                        name={low_field.name}
                        type="number"
                        min={config.minimum}
                        max={config.maximum}
                        disabled={!enabled || low_field.disabled}
                        value={low_field.value}
                        onBlur={low_field.onBlur}
                        onChange={(ev) => {
                            low_field.onChange(parseInt(ev.target.value, 10));
                        }}
                    />
                </FormControl>
            </FormItem>;
        }}/>
        <FormField control={form.control} name={`custom_fields.${index}.value.high`} render={({field: high_field}) => {
            return <FormItem className="w-1/2">
                <FormLabel>High</FormLabel>
                <FormControl>
                    <Input
                        ref={high_field.ref}
                        name={high_field.name}
                        type="number"
                        min={config.minimum}
                        max={config.maximum}
                        disabled={!enabled || high_field.disabled}
                        value={high_field.value}
                        onBlur={high_field.onBlur}
                        onChange={(ev) => {
                            high_field.onChange(parseInt(ev.target.value, 10));
                        }}
                    />
                </FormControl>
            </FormItem>;
        }}/>
    </div>;
}

export interface FloatConfigProps {
    id: string,
    config: custom_field.FloatType | custom_field.FloatRangeType,
    index: number,
}

export function FloatConfig({config, index}: FloatConfigProps) {
    const form = useFormContext<JournalForm>();

    return <div className="space-y-4">
        <div className="flex flex-nowrap gap-x-4">
            <FormField control={form.control} name={`custom_fields.${index}.config.minimum`} render={({field: min_field}) => {
                return <FormItem className="w-1/2">
                    <div className="flex flex-row flex-nowrap items-center gap-x-2">
                        <FormControl>
                            <Checkbox checked={min_field.value != null} onCheckedChange={(v) => {
                                min_field.onChange(v ? 0 : null)
                            }}/>
                        </FormControl>
                        <FormLabel>Minimum</FormLabel>
                    </div>
                    <FormControl>
                        <Input
                            ref={min_field.ref}
                            disabled={min_field.disabled || min_field.value == null}
                            name={min_field.name}
                            value={min_field.value ?? 0}
                            className="w-1/2"
                            type="number"
                            step="any"
                            onBlur={min_field.onBlur}
                            onChange={ev => {
                                min_field.onChange(parseFloat(ev.target.value));
                            }}
                        />
                    </FormControl>
                    <FormDescription>
                        The minimum value that a float value can be (inclusive)
                    </FormDescription>
                </FormItem>;
            }}/>
            <FormField control={form.control} name={`custom_fields.${index}.config.maximum`} render={({field: max_field}) => {
                return <FormItem className="w-1/2">
                    <div className="flex flex-row flex-nowrap items-center gap-x-2">
                        <FormControl>
                            <Checkbox checked={max_field.value != null} onCheckedChange={(v) => {
                                max_field.onChange(v ? 10 : null)
                            }}/>
                        </FormControl>
                        <FormLabel>Maximum</FormLabel>
                    </div>
                    <FormControl>
                        <Input
                            ref={max_field.ref}
                            disabled={max_field.disabled || max_field.value == null}
                            name={max_field.name}
                            value={max_field.value ?? 0}
                            className="w-1/2"
                            type="number"
                            step="any"
                            onBlur={max_field.onBlur}
                            onChange={ev => {
                                max_field.onChange(parseFloat(ev.target.value));
                            }}
                        />
                    </FormControl>
                    <FormDescription>
                        The maximum value that a float value can be (inclusive)
                    </FormDescription>
                </FormItem>
            }}/>
        </div>
        <div className="flex flex-row flex-nowrap gap-x-4">
            <FormField control={form.control} name={`custom_fields.${index}.config.step`} render={({field: step_field}) => {
                return <FormItem className="w-1/2">
                    <FormLabel>Step</FormLabel>
                    <FormControl>
                        <Input
                            ref={step_field.ref}
                            disabled={step_field.disabled}
                            name={step_field.name}
                            value={step_field.value}
                            className="w-1/2"
                            type="number"
                            step="any"
                            onBlur={step_field.onBlur}
                            onChange={ev => {
                                step_field.onChange(parseFloat(ev.target.value));
                            }}
                        />
                    </FormControl>
                    <FormDescription>
                        The amount to increase / decrease a number by
                    </FormDescription>
                </FormItem>
            }}/>
            <FormField control={form.control} name={`custom_fields.${index}.config.precision`} render={({field: prec_field}) => {
                return <FormItem className="w-1/2">
                    <FormLabel>Precision</FormLabel>
                    <FormControl>
                        <Input
                            ref={prec_field.ref}
                            disabled={prec_field.disabled}
                            name={prec_field.name}
                            value={prec_field.value}
                            className="w-1/2"
                            type="number"
                            step="1"
                            min="1"
                            onBlur={prec_field.onBlur}
                            onChange={ev => {
                                prec_field.onChange(parseInt(ev.target.value, 10));
                            }}
                        />
                    </FormControl>
                    <FormDescription>
                        The number of decimal places to display when entering a value
                    </FormDescription>
                </FormItem>
            }}/>
        </div>
    </div>;
}

export interface FloatValueProps {
    index: number,
    config: custom_field.FloatType,
}

export function FloatValue({index, config}: FloatValueProps) {
    const form = useFormContext<EntryForm>();
    const enabled = form.watch(`custom_fields.${index}.enabled`);

    return <FormField control={form.control} name={`custom_fields.${index}.value.value`} render={({field: flt_field}) => {
        return <FormItem className="w-1/2">
            <FormLabel>Value</FormLabel>
            <FormControl>
                <Input
                    ref={flt_field.ref}
                    name={flt_field.name}
                    type="number"
                    min={config.minimum}
                    max={config.maximum}
                    step={config.step}
                    disabled={!enabled || flt_field.disabled}
                    value={flt_field.value ?? 0.0}
                    onBlur={flt_field.onBlur}
                    onChange={(ev) => {
                        flt_field.onChange(parseFloat(ev.target.value));
                    }}
                />
            </FormControl>
        </FormItem>;
    }}/>;
}

export interface FloatRangeValueProps {
    index: number,
    config: custom_field.FloatRangeType,
}

export function FloatRangeValue({index, config}: FloatRangeValueProps) {
    const form = useFormContext<EntryForm>();
    const enabled = form.watch(`custom_fields.${index}.enabled`);

    return <div className="flex flow-row flex-nowrap items-center gap-x-4">
        <FormField control={form.control} name={`custom_fields.${index}.value.low`} render={({field: low_field}) => {
            return <FormItem className="w-1/2">
                <FormLabel>Low</FormLabel>
                <FormControl>
                    <Input
                        ref={low_field.ref}
                        name={low_field.name}
                        type="number"
                        min={config.minimum}
                        max={config.maximum}
                        step={config.step}
                        disabled={!enabled || low_field.disabled}
                        value={low_field.value}
                        onBlur={low_field.onBlur}
                        onChange={(ev) => {
                            low_field.onChange(parseFloat(ev.target.value));
                        }}
                    />
                </FormControl>
            </FormItem>;
        }}/>
        <FormField control={form.control} name={`custom_fields.${index}.value.high`} render={({field: high_field}) => {
            return <FormItem className="w-1/2">
                <FormLabel>High</FormLabel>
                <FormControl>
                    <Input
                        ref={high_field.ref}
                        name={high_field.name}
                        type="number"
                        min={config.minimum}
                        max={config.maximum}
                        step={config.step}
                        disabled={!enabled || high_field.disabled}
                        value={high_field.value}
                        onBlur={high_field.onBlur}
                        onChange={(ev) => {
                            high_field.onChange(parseFloat(ev.target.value));
                        }}
                    />
                </FormControl>
            </FormItem>;
        }}/>
    </div>;
}

export interface TimeRangeConfigProps {
    id: string,
    config: custom_field.TimeRangeType,
    index: number,
}

export function TimeRangeConfig({config, index}: TimeRangeConfigProps) {
    const form = useFormContext<JournalForm>();

    return <FormField control={form.control} name={`custom_fields.${index}.config.show_diff`} render={({field: sd_field}) => {
        return <FormItem className="w-3/4 flex flex-row items-start justify-between">
            <div className="space-y-0.5">
                <FormLabel>Show Difference</FormLabel>
                <FormDescription>
                    When enabled instead of showing the start and end times it will display a difference between the two times
                </FormDescription>
            </div>
            <FormControl>
                <Switch {...sd_field} checked={sd_field.value} onCheckedChange={sd_field.onChange}/>
            </FormControl>
        </FormItem>;
    }}/>;
}

export interface TimeRangeValueProps {
    index: number,
    config: custom_field.TimeRangeType,
}

export function TimeRangeValue({index, config}: TimeRangeValueProps) {
    const form = useFormContext<EntryForm>();
    const enabled = form.watch(`custom_fields.${index}.enabled`);

    return <div className="flex flow-row flex-nowrap items-center gap-x-4">
        <FormField control={form.control} name={`custom_fields.${index}.value.low`} render={({field: low_field}) => {
            let date_value = typeof low_field.value === "string" ? new Date(low_field.value) : low_field.value as Date;

            return <FormItem className="w-1/2">
                <FormLabel>Start</FormLabel>
                <Popover>
                    <PopoverTrigger asChild>
                        <FormControl>
                            <Button
                                variant="outline"
                                className="w-full justify-start text-left front-normal truncate"
                                disabled={!enabled}
                            >
                                {format(date_value, "yyyy/LL/dd, HH:mm:ss")}
                            </Button>
                        </FormControl>
                    </PopoverTrigger>
                    <PopoverContent className="w-auto p-0" aligh="start">
                        <div className="sm:flex">
                            <Calendar
                                name={low_field.name}
                                mode="single"
                                selected={date_value}
                                onBlur={low_field.onBlur}
                                onSelect={(selected) => {
                                    selected.setHours(date_value.getHours());
                                    selected.setMinutes(date_value.getMinutes());
                                    selected.setSeconds(date_value.getSeconds());

                                    low_field.onChange(selected);
                                }}
                                disabled={(date) => {
                                    return date > new Date() || date < new Date("1900-01-01");
                                }}
                            />
                            <TimePicker value={date_value} on_change={low_field.onChange} />
                        </div>
                    </PopoverContent>
                </Popover>
            </FormItem>;
        }}/>
        <FormField control={form.control} name={`custom_fields.${index}.value.high`} render={({field: high_field}) => {
            let date_value = typeof high_field.value === "string" ? new Date(high_field.value) : high_field.value as Date;

            return <FormItem className="w-1/2">
                <FormLabel>End</FormLabel>
                <Popover>
                    <PopoverTrigger asChild>
                        <FormControl>
                            <Button
                                variant="outline"
                                className="w-full justify-start text-left front-normal truncate"
                                disabled={!enabled}
                            >
                                {format(high_field.value, "yyyy/LL/dd, HH:mm:ss")}
                            </Button>
                        </FormControl>
                    </PopoverTrigger>
                    <PopoverContent className="w-auto p-0" aligh="start">
                        <div className="sm:flex">
                            <Calendar
                                name={high_field.name}
                                mode="single"
                                selected={date_value}
                                onBlur={high_field.onBlur}
                                onSelect={(selected) => {
                                    selected.setHours(date_value.getHours());
                                    selected.setMinutes(date_value.getMinutes());
                                    selected.setSeconds(date_value.getSeconds());

                                    high_field.onChange(selected);
                                }}
                                disabled={(date) => {
                                    return date > new Date() || date < new Date("1900-01-01");
                                }}
                            />
                            <TimePicker value={date_value} on_change={high_field.onChange}/>
                        </div>
                    </PopoverContent>
                </Popover>
            </FormItem>;
        }}/>
    </div>;
}
