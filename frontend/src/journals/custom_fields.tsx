import { useFieldArray, useFormContext } from "react-hook-form";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
    FormControl,
    FormDescription,
    FormField,
    FormItem,
    FormLabel,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { custom_field } from "@/journals/api";
import { JournalForm } from "@/journals/forms";

export interface CustomFieldProps {
    
}

export function CustomField({}: CustomFieldProps) {
    return <div>
        I am a custom field
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
                <div className="flex flex-row flex-nowrap items-center gap-x-4">
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
                <div className="flex flex-row flex-nowrap items-center gap-x-4">
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
                    <div className="flex flex-row flex-nowrap items-center gap-x-4">
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
                    <div className="flex flex-row flex-nowrap items-center gap-x-4">
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
