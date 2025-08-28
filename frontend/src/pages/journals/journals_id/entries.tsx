import { Day, endOfMonth, endOfWeek, format, parse, startOfMonth, startOfWeek } from "date-fns";
import { useState, useMemo, useEffect, useCallback } from "react";
import { Link, useParams, useSearchParams } from "react-router-dom";
import { Plus, Search, ChevronUp, ChevronDown, CalendarIcon, RefreshCw, ArrowLeft, ArrowRight, LoaderCircle } from "lucide-react";
import { useForm, FormProvider, SubmitHandler,  } from "react-hook-form";
import { useQuery } from "@tanstack/react-query";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import {
    FormControl,
    FormField,
    FormItem,
    FormLabel,
    FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { CenterPage } from "@/components/ui/page";
import {
    DataTable,
    ColumnDef,
} from "@/components/ui/table";
import {
    EntryPartial,
    custom_field,
} from "@/journals/api";
import { CustomFieldEntryCell } from "@/journals/custom_fields";
import { req_api_json } from "@/net";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Checkbox } from "@/components/ui/checkbox";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { cn } from "@/utils";
import { date_to_naive_date, DAY_NAMES, MINUTE, MONTH_NAMES, naive_date_to_date, same_date } from "@/time";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { use_date } from "@/components/hooks/timers";
import { Separator } from "@/components/ui/separator";
import { toast } from "sonner";

interface CustomFieldPartial {
    id: number,
    name: string,
    description: string | null,
    config: custom_field.Type,
}

interface SearchEntriesResults {
    entries: EntryPartial[],
    custom_fields: CustomFieldPartial[]
}

enum EntriesView {
    Table = 0,
    Calendar = 1,
}

function search_entries_query_key(journals_id: string, query: string): ["search_entries", string, string] {
    return ["search_entries", journals_id, query];
}

function get_view(search_params: URLSearchParams): EntriesView {
    let view = search_params.get("view");

    if (view != null) {
        switch (view.toLowerCase()) {
            case "calendar":
                return EntriesView.Calendar;
        }
    }

    return EntriesView.Table;
}

export function Entries() {
    const { journals_id } = useParams();

    if (journals_id == null) {
        throw new Error("missing journals_id param");
    }

    const [search_params, _] = useSearchParams();
    const [view_type, set_view_type] = useState<EntriesView>(get_view(search_params));

    useEffect(() => {
        set_view_type(get_view(search_params));
    }, [search_params])

    switch (view_type) {
        case EntriesView.Table:
            return <EntriesTable journals_id={journals_id}/>;
        case EntriesView.Calendar:
            return <EntriesCalendar journals_id={journals_id}/>;
    }
}

interface EntriesCalendarProps {
    journals_id: string
}

let start_of_week: Day = 0;

function EntriesCalendar({journals_id}: EntriesCalendarProps) {
    let today = use_date();
    const [search_params, set_search_params] = useSearchParams();

    let [[year, month], set_date_tuple] = useState<[number, number]>(() => {
        let date = search_params.get("date");
        let now = new Date();

        if (date != null) {
            let parsed = parse(date, "yyyy-MM", now);

            if (!isNaN(parsed.getTime())) {
                return [parsed.getFullYear(), parsed.getMonth()];
            }
        }

        return [now.getFullYear(), now.getMonth()];
    });

    const {data, isFetching, error} = useQuery({
        queryKey: ["calendar_entries", journals_id, year, month] as [string, string, number, number],
        queryFn: async ({queryKey}) => {
            let {entries, custom_fields} = await req_api_json<SearchEntriesResults>(
                "GET",
                `/journals/${queryKey[1]}/entries?date=${queryKey[2]}-${(queryKey[3] + 1).toString().padStart(2, '0')}`
            );

            return create_calendar_month({year: queryKey[2], month: queryKey[3], entries});
        },
        placeholderData: (prev_data, prev_query) => prev_data,
    });

    let week_days = useMemo(() => {
        let rtn = [];

        for (let count = 0; count < 7; count += 1) {
            let index = (count + start_of_week) % 7;

            rtn.push(<div key={index} className="text-center">
                {DAY_NAMES[index]}
            </div>);
        }

        return rtn;
    }, []);

    let update_date = useCallback(({next_year = year, next_month = month}: {next_year?: number, next_month?: number}) => {
        set_date_tuple([next_year, next_month]);
        set_search_params(curr => {
            let now = new Date();

            if (next_year === now.getFullYear() && next_month === now.getMonth()) {
                curr.delete("date");
            } else {
                curr.set("date", `${next_year}-${(next_month + 1).toString(10).padStart(2, '0')}`);
            }

            return curr;
        });
    }, [year, month]);

    let prev_month = useCallback(() => {
        let next_month = month - 1;
        let next_year = year;

        if (next_month === -1) {
            next_month = 11;
            next_year -= 1;
        }

        update_date({next_year, next_month});
    }, [year, month]);

    let next_month = useCallback(() => {
        let next_month = month + 1;
        let next_year = year;

        if (next_month === 12) {
            next_month = 0;
            next_year += 1;
        }

        update_date({next_year, next_month});
    }, [year, month]);

    return <CenterPage className="max-w-7xl pt-2">
        <div className="flex flex-row items-center">
            <Button type="button" variant="outline" size="icon" disabled={isFetching} onClick={prev_month}>
                <ArrowLeft/>
            </Button>
            <div className="flex-1"/>
            <div className="flex flex-row gap-x-2">
                <MonthSelect
                    value={month}
                    disabled={isFetching}
                    on_change={value => update_date({next_month: value})}
                />
                <YearSelect
                    value={year}
                    lower={1900}
                    upper={today.getFullYear()}
                    disabled={isFetching}
                    on_change={value => update_date({next_year: value})}
                />
                <Button
                    type="button"
                    variant="outline"
                    disabled={isFetching || (year === today.getFullYear() && month == today.getMonth())}
                    onClick={() => update_date({next_year: today.getFullYear(), next_month: today.getMonth()})}
                >
                    Today
                </Button>
            </div>
            <div className="flex-1 flex flex-row items-center justify-start pl-2 gap-x-2">
                {isFetching ?
                    <>
                        <LoaderCircle className="animate-spin"/>
                        <span>Loading...</span>
                    </>
                    :
                    null
                }
            </div>
            <Button type="button" variant="outline" size="icon" disabled={isFetching} onClick={next_month}>
                <ArrowRight/>
            </Button>
        </div>
        <div className="grid grid-cols-7 gap-4">
            {week_days}
            {data != null ?
                data.map(({date, key, is_spacer, record}) => is_spacer ?
                    <div key={key}/> :
                    <CalendarCell
                        key={key}
                        date={key}
                        is_today={same_date(date, today)}
                        record={record}
                        disable={isFetching}
                    />
                )
                :
                null
            }
        </div>
    </CenterPage>;
}

interface CreateCalendarList {
    year: number,
    month: number,
    start_of_week?: Day,
    entries: EntryPartial[],
}

function create_calendar_month({
    year,
    month,
    entries,
    start_of_week = 0,
}: CreateCalendarList) {
    let month_start = startOfMonth(new Date(year, month));
    let month_end = endOfMonth(new Date(year, month));
    let rtn = [];
    let list_index = entries.length - 1;

    for (let iter = startOfWeek(month_start, {weekStartsOn: start_of_week}); iter < month_start; iter.setDate(iter.getDate() + 1)) {
        rtn.push({
            date: new Date(iter),
            key: date_to_naive_date(iter),
            is_spacer: true,
            record: null,
        });
    }

    for (let iter = month_start; iter <= month_end; iter.setDate(iter.getDate() + 1)) {
        let key = date_to_naive_date(iter);
        let record = null;

        if (list_index >= 0 && entries[list_index].date === key) {
            record = entries[list_index];
            list_index -= 1;
        }

        rtn.push({
            date: new Date(iter),
            key,
            is_spacer: false,
            record,
        });
    }

    let week_end = endOfWeek(month_end, {weekStartsOn: start_of_week});
    let end = new Date(month_end);
    end.setDate(end.getDate() + 1);

    for (let iter = end; iter <= week_end; iter.setDate(iter.getDate() + 1)) {
        rtn.push({
            date: new Date(iter),
            key: date_to_naive_date(iter),
            is_spacer: true,
            record: null,
        });
    }

    if (list_index >= 0) {
        console.warn("did not cover all records returned");

        for (let index = list_index; index < entries.length; index += 1) {
            console.log(entries[index]);
        }
    }

    return rtn;
}

interface CalendarCellProps {
    date: string,
    is_today: boolean,
    disable: boolean,
    record: EntryPartial | null,
}

function CalendarCell({date, is_today, record, disable}: CalendarCellProps) {
    if (record == null) {
        return <div
            className={cn("border rounded-lg p-2 h-48 text-muted-foreground hover:bg-secondary transition-colors", {
                "border-foreground": is_today,
            })}
        >
            {disable ? date : <Link to={`./${date}`}>{date}</Link>}
        </div>;
    }

    let list = [];

    for (let tag in record.tags) {
        let value = record.tags[tag];

        if (value != null) {
            list.push(<Tooltip key={tag}>
                <TooltipTrigger>
                    <Badge variant="outline">{tag}</Badge>
                </TooltipTrigger>
                <TooltipContent>
                    <p>{value}</p>
                </TooltipContent>
            </Tooltip>);
        } else {
            list.push(<Badge key={tag} variant="outline">{tag}</Badge>);
        }
    }

    let content = null;

    if (list.length > 0) {
        content = <div className="flex flex-row flex-wrap gap-1">{list}</div>;
    }

    return <div
        className={cn("space-y-2 border rounded-lg h-48 hover:bg-secondary transition-colors", {
            "border-foreground": is_today,
        })}
    >
        <div className="px-2 pt-2">
            {disable ?
                record.date
                :
                <Link to={`./${record.date}`}>{record.date}</Link>
            }
        </div>
        <Separator/>
        <div className="px-2 pb-2">
            {record.title != null ? <h4 className="text-lg truncate">{record.title}</h4> : null}
            {content}
        </div>
    </div>
}

interface YearSelectProps {
    value: number,
    lower: number,
    upper: number,
    disabled: boolean,
    on_change: (value: number) => void,
}

function YearSelect({value, lower, upper, disabled, on_change}: YearSelectProps) {
    let years = useMemo(() => {
        let year_list = [];

        for (let index = upper; index >= lower; index -= 1) {
            year_list.push(<SelectItem key={index} value={index.toString()}>{index}</SelectItem>);
        }

        return year_list;
    }, [lower, upper]);

    return <Select
        onValueChange={value => on_change(parseInt(value, 10))}
        value={value.toString(10)}
        disabled={disabled}
    >
        <SelectTrigger>
            <SelectValue/>
        </SelectTrigger>
        <SelectContent>{years}</SelectContent>
    </Select>;
}

interface MonthSelectProps {
    value: number,
    disabled: boolean,
    on_change: (value: number) => void,
}

function MonthSelect({value, disabled, on_change}: MonthSelectProps) {
    let months = useMemo(() => MONTH_NAMES.map((name, index) => {
        return <SelectItem key={index} value={index.toString()}>{name}</SelectItem>;
    }), []);

    return <Select
        onValueChange={value => on_change(parseInt(value, 10))}
        value={value.toString(10)}
        disabled={disabled}
    >
        <SelectTrigger>
            <SelectValue/>
        </SelectTrigger>
        <SelectContent>{months}</SelectContent>
    </Select>;
}

interface SearchQuery {
    start_date: Date | null,
    end_date: Date | null,
}

interface EntriesTableProps {
    journals_id: string,
}

function EntriesTable({journals_id}: EntriesTableProps) {
    const [search_params, set_search_params] = useSearchParams();

    const {data, isFetching, error, refetch} = useQuery<SearchEntriesResults, Error, SearchEntriesResults, ReturnType<typeof search_entries_query_key>>({
        queryKey: search_entries_query_key(journals_id, search_params.toString()),
        queryFn: async ({queryKey}) => {
            let [_, journals_id, query] = queryKey;

            return await req_api_json<SearchEntriesResults>("GET", `/journals/${journals_id}/entries?${query}`);
        }
    });

    const {entries = [], custom_fields = []} = data ?? {};

    const columns = useMemo(() => {
        let columns: ColumnDef<EntryPartial>[] = [
            {
                header: "Date",
                cell: ({ row }) => {
                    return <Link to={`/journals/${row.original.journals_id}/entries/${row.original.id}`}>
                        {row.original.date}
                    </Link>;
                }
            },
            {
                accessorKey: "title",
                header: "Title",
            },
        ];

        for (let field of custom_fields) {
            columns.push({
                header: field.name,
                cell: ({ row }) => {
                    if (!(field.id in row.original.custom_fields)) {
                        return null;
                    }

                    return <CustomFieldEntryCell
                        value={row.original.custom_fields[field.id]}
                        config={field.config}
                    />;
                }
            });
        }

        columns.push(
            {
                header: "Tags",
                cell: ({ row }) => {
                    let list = [];

                    for (let tag in row.original.tags) {
                        let value = row.original.tags[tag];

                        if (value != null) {
                            list.push(<Tooltip key={tag}>
                                <TooltipTrigger>
                                    <Badge variant="outline">{tag}</Badge>
                                </TooltipTrigger>
                                <TooltipContent>
                                    <p>{value}</p>
                                </TooltipContent>
                            </Tooltip>);
                        } else {
                            list.push(<Badge key={tag} variant="outline">{tag}</Badge>);
                        }
                    }

                    return <div className="max-w-96 flex flex-row flex-wrap gap-1">{list}</div>;
                }
            }
        );

        return columns;
    }, [custom_fields]);

    return <CenterPage className="max-w-6xl">
        <SearchHeader
            query={{
                start_date: naive_date_to_date(decodeURIComponent(search_params.get("start_date") ?? "")),
                end_date: naive_date_to_date(decodeURIComponent(search_params.get("end_date") ?? "")),
            }}
            journals_id={journals_id}
            is_fetching={isFetching}
            on_refetch={refetch}
            on_search={data => {
                set_search_params(prev => {
                    if (data.start_date != null) {
                        prev.set("start_date", date_to_naive_date(data.start_date));
                    } else {
                        prev.delete("start_date");
                    }

                    if (data.end_date != null) {
                        prev.set("end_date", date_to_naive_date(data.end_date));
                    } else {
                        prev.delete("end_date");
                    }

                    return prev;
                });
            }}
        />
        <DataTable
            columns={columns}
            data={entries}
            empty={isFetching ? "Loading..." : "No Journal Entries"}
        />
    </CenterPage>;
}

interface SearchHeaderProps {
    query: SearchQuery,
    journals_id: string,
    is_fetching: boolean,
    on_search: (data: SearchQuery) => void,
    on_refetch: () => void,
}

interface SearchForm {
    start_date: {
        enabled: boolean,
        value: Date,
    },
    end_date: {
        enabled: boolean,
        value: Date,
    },
}

function get_search_form(query: SearchQuery): SearchForm {
    return {
        start_date: {
            enabled: query.start_date != null,
            value: query.start_date ?? new Date(),
        },
        end_date: {
            enabled: query.end_date != null,
            value: query.end_date ?? new Date(),
        },
    };
}

function SearchHeader({query, journals_id, is_fetching, on_search, on_refetch}: SearchHeaderProps) {
    const [view_options, set_view_options] = useState(false);

    const form = useForm<SearchForm>({
        defaultValues: get_search_form(query),
    });

    const on_submit: SubmitHandler<SearchForm> = (data, ev) => {
        let is_valid = true;

        if (data.start_date.enabled && data.end_date.enabled) {
            if (data.start_date.value > data.end_date.value) {
                form.setError("end_date.value", {
                    message: "End date must be greater than the start date"
                });

                is_valid = false;
            }
        }

        if (!is_valid) {
            return;
        }

        on_search({
            start_date: data.start_date.enabled ? data.start_date.value : null,
            end_date: data.end_date.enabled ? data.end_date.value : null,
        });
    };

    useEffect(() => {
        form.reset(get_search_form(query));
    }, [query]);

    return <FormProvider {...form}>
        <form className="sticky top-0 bg-background z-10 py-4 border-b" onSubmit={form.handleSubmit(on_submit)}>
            <Collapsible open={view_options} className="space-y-4" onOpenChange={set_view_options}>
                <div className="flex flex-row flex-nowrap items-center gap-x-4">
                    <div className="w-1/2 relative">
                        <Input type="text" placeholder="Search" disabled={is_fetching}/>
                    </div>
                    {form.formState.isDirty ?
                        <Button type="submit" variant="secondary" size="icon" disabled={is_fetching}>
                            <Search/>
                        </Button>
                        :
                        <Button type="button" variant="secondary" size="icon" disabled={is_fetching} onClick={() => on_refetch()}>
                            <RefreshCw/>
                        </Button>
                    }
                    <Link to={`/journals/${journals_id}/entries/new`}>
                        <Button type="button"><Plus/>New Entry</Button>
                    </Link>
                    {is_fetching ? <span>Loading...</span> : null}
                    <div className="flex-1"/>
                    <CollapsibleTrigger asChild>
                        <Button type="button" variant="ghost" size="icon">
                            {view_options ? <ChevronUp/> : <ChevronDown/>}
                        </Button>
                    </CollapsibleTrigger>
                </div>
                <CollapsibleContent className="grid grid-cols-3 gap-4">
                    <div className="space-y-2 pt-1">
                        <FormField control={form.control} name="start_date.enabled" render={({field}) => {
                            return <FormItem className="flex flex-row flex-nowrap items-center gap-2 space-y-0">
                                <FormControl>
                                    {/* we should be getting only boolean and not indeterminate */}
                                    <Checkbox checked={field.value} onCheckedChange={field.onChange}/>
                                </FormControl>
                                <FormLabel>Start Date</FormLabel>
                            </FormItem>
                        }}/>
                        <FormField control={form.control} name="start_date.value" render={({field}) => {
                            let enabled = form.getValues("start_date.enabled");

                            return <FormItem>
                                <Popover>
                                    <PopoverTrigger asChild>
                                        <FormControl>
                                            <Button
                                                type="button"
                                                variant="outline"
                                                disabled={!enabled}
                                                className={cn("w-full", {"text-muted-foreground": !enabled})}
                                            >
                                                {enabled ? format(field.value, "PPP") : "Pick a date"}
                                                <CalendarIcon className="ml-auto h-4 w-4"/>
                                            </Button>
                                        </FormControl>
                                    </PopoverTrigger>
                                    <PopoverContent className="w-auto p-0" align="start">
                                        <Calendar mode="single" selected={field.value} onSelect={field.onChange} captionLayout="dropdown"/>
                                    </PopoverContent>
                                </Popover>
                                <FormMessage/>
                            </FormItem>
                        }}/>
                    </div>
                    <div  className="space-y-2 pt-1">
                        <FormField control={form.control} name="end_date.enabled" render={({field}) => {
                            return <FormItem className="flex flex-row flex-nowrap items-center gap-x-2 space-y-0">
                                <FormControl>
                                    <Checkbox checked={field.value} onCheckedChange={field.onChange}/>
                                </FormControl>
                                <FormLabel>End Date</FormLabel>
                            </FormItem>
                        }}/>
                        <FormField control={form.control} name="end_date.value" render={({field}) => {
                            let enabled = form.getValues("end_date.enabled");

                            return <FormItem>
                                <Popover>
                                    <PopoverTrigger asChild>
                                        <FormControl>
                                            <Button
                                                type="button"
                                                variant="outline"
                                                disabled={!enabled}
                                                className={cn("w-full", {"text-muted-foreground": !enabled})}
                                            >
                                                {enabled ? format(field.value, "PPP") : "Pick a date"}
                                                <CalendarIcon className="ml-auto h-4 w-4"/>
                                            </Button>
                                        </FormControl>
                                    </PopoverTrigger>
                                    <PopoverContent className="w-auto p-0" align="start">
                                        <Calendar mode="single" selected={field.value} onSelect={field.onChange} captionLayout="dropdown"/>
                                    </PopoverContent>
                                </Popover>
                                <FormMessage/>
                            </FormItem>
                        }}/>
                    </div>
                </CollapsibleContent>
            </Collapsible>
        </form>
    </FormProvider>;
}