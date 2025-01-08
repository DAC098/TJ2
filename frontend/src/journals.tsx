import { useState, useEffect, Fragment } from "react";
import { useForm, useFieldArray, useFormContext, FormProvider, SubmitHandler,  } from "react-hook-form";
import { Routes, Route, Link, useParams, useNavigate } from "react-router-dom";
import { Plus, Save, Trash, RefreshCcw, Search, Check, Pencil, ArrowLeft } from "lucide-react";

import { send_json } from "@/net";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
    FormControl,
    FormDescription,
    FormField,
    FormItem,
    FormLabel,
    FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { CenterPage } from "@/components/ui/page";
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import {
    Sheet,
    SheetContent,
    SheetDescription,
    SheetHeader,
    SheetTitle,
    SheetTrigger,
} from "@/components/ui/sheet";
import { Switch } from "@/components/ui/switch";
import {
    DataTable,
    ColumnDef,
} from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
import {
    JournalPartial,
    JournalFull,
    get_journals,
    get_journal,
    custom_field,
} from "@/journals/api";
import {
    JournalForm,
    blank_journal_form,
} from "@/journals/forms";
import { Entry, Entries } from "@/journals/entries";

export function JournalRoutes() {
    return <Routes>
        <Route index element={<JournalsIndex />}/>
        <Route path="/:journals_id" element={<Journal />}/>
        <Route path="/:journals_id/entries" element={<Entries />}/>
        <Route path="/:journals_id/entries/:entries_id" element={<Entry />}/>
    </Routes>;
}

function JournalsIndex() {
    return <CenterPage className="flex items-center justify-center h-full">
        <div className="w-1/2 flex flex-col flex-nowrap items-center">
            <h2 className="text-2xl">Nothing to see here</h2>
            <p>Select a journal on the sidebar to view its entries</p>
        </div>
    </CenterPage>;
}

function journal_to_form(journal: JournalFull) {
    let custom_fields = [];

    for (let field of journal.custom_fields) {
        custom_fields.push({
            _id: field.id,
            uid: field.uid,
            name: field.name,
            order: field.order,
            config: field.config,
            description: field.description ?? "",
        });
    }

    return {
        name: journal.name,
        description: journal.description ?? "",
        custom_fields,
    };
}

interface JournalHeaderProps {
    journals_id: string,
    on_delete: () => void,
}

function JournalHeader({journals_id, on_delete}: JournalHeaderProps) {
    const navigate = useNavigate();
    const form = useFormContext<JournalForm>();

    return <div className="top-0 sticky flex flex-row flex-nowrap gap-x-4">
        <Link to="/journals">
            <Button type="button" variant="ghost" size="icon">
                <ArrowLeft/>
            </Button>
        </Link>
        <FormField control={form.control} name="name" render={({ field }) => {
            return <FormItem>
                <FormControl>
                    <Input type="text" placeholder="Name" {...field}/>
                </FormControl>
            </FormItem>
        }}/>
        <Button type="submit">
            Save<Save/>
        </Button>
        {journals_id !== "new" ?
            <Button
                type="button"
                variant="destructive"
                disabled
                onClick={() => {
                    on_delete();
                }}
            >
                Delete
                <Trash/>
            </Button>
            :
            null
        }
    </div>;
}

function Journal() {
    const { journals_id } = useParams();
    const navigate = useNavigate();

    const form = useForm<JournalForm>({
        defaultValues: async () => {
            if (journals_id === "new") {
                return blank_journal_form();
            }

            try {
                let result = await get_journal(journals_id);

                if (result != null) {
                    return journal_to_form(result);
                }
            } catch (err) {
                console.error("failed to retrieve journal", err);
            }

            return blank_journal_form();
        }
    });

    const create_journal = async (data: JournalForm) => {
        let custom_fields = [];

        for (let field of data.custom_fields) {
            let desc = field.description.trim();

            custom_fields.push({
                name: field.name,
                order: field.order,
                config: field.config,
                description: desc.length === 0 ? null : desc
            });
        }

        let desc = data.description.trim();

        let res = await send_json("POST", "/journals", {
            name: data.name,
            description: desc.length === 0 ? null : desc,
            custom_fields,
        });

        switch (res.status) {
        case 200:
            return await res.json();
        case 400:
            let json = await res.json();

            console.error("failed to create journal", json);
            break;
        case 403:
            console.error("you do not have permission to create journals");
            break;
        default:
            console.warn("unhandled response status code");
            break;
        }

        return null;
    };

    const update_journal = async (data: JournalForm) => {
        let custom_fields = [];

        for (let field of data.custom_fields) {
            let desc = field.description.trim();
            let obj = {
                name: field.name,
                order: field.order,
                config: field.config,
                description: desc.length === 0 ? null : desc
            };

            if (field._id != null) {
                obj["id"] = field._id;
            }

            custom_fields.push(obj);
        }

        let description = data.description.trim();

        let res = await send_json("PATCH", `/journals/${journals_id}`, {
            name: data.name,
            description: description.length === 0 ? null : description,
            custom_fields,
        });

        switch (res.status) {
        case 200:
            return true;
        case 400:
            let json = await res.json();

            console.error("failed to update journal", json);
            break;
        case 403:
            console.error("you do not have permission to update journals");
            break;
        case 404:
            console.error("journal not found");
            break;
        default:
            console.warn("unhandled response status code");
            break;
        }

        return false;
    };

    const on_delete = () => {};

    const on_submit: SubmitHandler<JournalForm> = async (data, event) => {
        if (journals_id === "new") {
            try {
                let created = await create_journal(data);

                if (created == null) {
                    return;
                }

                form.reset(journal_to_form(created));

                navigate(`/journals/${created.id}`);
            } catch (err) {
                console.error("error when creating new journal", err);
            }
        } else {
            try {
                if (await update_journal(data)) {
                    form.reset(data);
                }
            } catch (err) {
                console.error("error when updating journal", err);
            }
        }
    };

    if (form.formState.isLoading) {
        return <CenterPage>
            loading journal
        </CenterPage>;
    }

    return <CenterPage>
        <FormProvider<JournalForm> {...form} children={
            <form onSubmit={form.handleSubmit(on_submit)} className="space-y-4">
                <JournalHeader journals_id={journals_id} on_delete={on_delete}/>
                <FormField control={form.control} name="description" render={({ field }) => {
                    return <FormItem className="w-1/2">
                        <FormLabel>Description</FormLabel>
                        <FormControl>
                            <Textarea type="text" {...field}/>
                        </FormControl>
                    </FormItem>
                }}/>
                <CustomFieldList />
            </form>
        }/>
    </CenterPage>;
}

interface CustomFieldListProps {
}

function CustomFieldList({}: CustomFieldListProps) {
    const form = useFormContext<JournalForm>();
    const custom_fields = useFieldArray<JournalForm, "custom_fields">({
        control: form.control,
        name: "custom_fields",
    });

    return <div className="space-y-4">
        <div className="flex flex-row flex-nowrap gap-x-4 items-center">
            Custom Fields
            <Button type="button" variant="secondary" onClick={() => {
                custom_fields.append({
                    _id: null,
                    uid: null,
                    name: "",
                    order: 0,
                    config: {
                        type: custom_field.TypeName.Integer,
                        minimum: null,
                        maximum: null,
                    },
                    description: "",
                });
            }}><Plus/>Add Field</Button>
        </div>
        {custom_fields.fields.map((field, index) => {
            let type_ui = null;
            let type_desc = null;

            switch (field.config.type) {
            case custom_field.TypeName.Integer:
                type_ui = <IntegerConfig id={field.id} config={field.config} index={index}/>;
                type_desc = <FormDescription>
                    Single whole number input that can have an optional minimum and maximum value.
                </FormDescription>;
                break;
            case custom_field.TypeName.IntegerRange:
                type_ui = <IntegerConfig id={field.id} config={field.config} index={index}/>;
                type_desc = <FormDescription>
                    Whole number input that can specify a range between a low and high value with an optional minimum and maximum value.
                </FormDescription>;
                break;
            case custom_field.TypeName.Float:
                type_ui = <FloatConfig id={field.id} config={field.config} index={index}/>;
                type_desc = <FormDescription>
                    Single decimal number input that can have an optional minimum and maximum value.
                    Can also specify the precision of the value and the step at which to increase that value by.
                </FormDescription>;
                break;
            case custom_field.TypeName.FloatRange:
                type_ui = <FloatConfig id={field.id} config={field.config} index={index}/>;
                type_desc = <FormDescription>
                    Decimal number input that can specify a range between a low and high value with an optional minimum and maximum value.
                    Can also specify the precision of the value and the step at which to increase that value by.
                </FormDescription>;
                break;
            case custom_field.TypeName.Time:
                type_desc = <FormDescription>
                    under consideration
                </FormDescription>;
                break;
            case custom_field.TypeName.TimeRange:
                type_ui = <TimeRangeConfig id={field.id} config={field.config} index={index}/>;
                type_desc = <FormDescription>
                    Time input that can specify a range between a low and high value.
                </FormDescription>;
                break;
            }

            return <Fragment key={field.id}>
                <Separator/>
                <div key={field.id} className="space-y-4">
                    <div className="flex flex-row flex-nowrap gap-x-4">
                        <FormField control={form.control} name={`custom_fields.${index}.name`} render={({field: name_field}) => {
                            return <FormItem className="w-1/2">
                                <FormControl>
                                    <Input type="text" placeholder="Custom Field Name" {...name_field}/>
                                </FormControl>
                            </FormItem>
                        }}/>
                        <Button type="button" variant="destructive" size="icon" onClick={() => {
                            custom_fields.remove(index);
                        }}><Trash/></Button>
                    </div>
                    <FormField control={form.control} name={`custom_fields.${index}.order`} render={({field: order_field}) => {
                        return <FormItem>
                            <FormLabel>Order</FormLabel>
                            <FormControl>
                                <Input
                                    ref={order_field.ref}
                                    disabled={order_field.disabled}
                                    name={order_field.name}
                                    value={order_field.value}
                                    className="w-1/4"
                                    type="number"
                                    min="0"
                                    max="100"
                                    onBlur={order_field.onBlur}
                                    onChange={ev => {
                                        order_field.onChange(parseInt(ev.target.value, 10));
                                    }}
                                />
                            </FormControl>
                            <FormDescription>
                                This will determine the display order of the fields when creating a new entry.
                                Higher order values have higher priority when being displayed.
                                If multiple fields have the same value then the name of the field will determine the sort order.
                            </FormDescription>
                        </FormItem>
                    }}/>
                    <FormField control={form.control} name={`custom_fields.${index}.description`} render={({field: desc_field}) => {
                        return <FormItem className="w-3/4">
                            <FormLabel>Description</FormLabel>
                            <FormControl>
                                <Textarea type="text" {...desc_field}/>
                            </FormControl>
                        </FormItem>
                    }}/>
                    <FormItem>
                        <FormLabel>Type</FormLabel>
                        <Select
                            defaultValue={field.config.type}
                            onValueChange={value => {
                                let curr = form.getValues(`custom_fields.${index}`);

                                custom_fields.update(index, {
                                    ...curr,
                                    config: custom_field.make_type(value),
                                });
                            }}
                        >
                            <FormControl>
                                <SelectTrigger className="w-1/4">
                                    <SelectValue placeholder="Type"/>
                                </SelectTrigger>
                            </FormControl>
                            <SelectContent>
                                <SelectItem value="Integer">Integer</SelectItem>
                                <SelectItem value="IntegerRange">Integer Range</SelectItem>
                                <SelectItem value="Float">Float</SelectItem>
                                <SelectItem value="FloatRange">Float Range</SelectItem>
                                <SelectItem value="Time">Time</SelectItem>
                                <SelectItem value="TimeRange">Time Range</SelectItem>
                            </SelectContent>
                        </Select>
                        {type_desc}
                    </FormItem>
                    {type_ui}
                </div>
            </Fragment>;
        })}
    </div>;
}

interface IntegerConfigProps {
    id: string,
    config: custom_field.IntegerType | custom_field.IntegerRangeType,
    index: number,
}

function IntegerConfig({config, index}:IntegerConfigProps) {
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

interface FloatConfigProps {
    id: string,
    config: custom_field.FloatType | custom_field.FloatRangeType,
    index: number,
}

function FloatConfig({config, index}: FloatConfigProps) {
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

interface TimeRangeConfigProps {
    id: string,
    config: custom_field.TimeRangeType,
    index: number,
}

function TimeRangeConfig({config, index}: TimeRangeConfigProps) {
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
