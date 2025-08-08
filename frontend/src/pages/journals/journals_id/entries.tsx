import { format, formatDistanceToNow } from "date-fns";
import { useState, useMemo, useEffect } from "react";
import { Link, useParams, useSearchParams } from "react-router-dom";
import { Plus, Search, ChevronUp, ChevronDown, CalendarIcon, RefreshCw } from "lucide-react";
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
import { date_to_naive_date, naive_date_to_date } from "@/time";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

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

interface SearchQuery {
    start_date: Date | null,
    end_date: Date | null,
}

function search_entries_query_key(journals_id: string, query: string): ["search_entries", string, string] {
    return ["search_entries", journals_id, query];
}

export function Entries() {
    const { journals_id } = useParams();
    const [search_params, set_search_params] = useSearchParams();

    if (journals_id == null) {
        throw new Error("missing journals_id param");
    }

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
                let params: any = {};

                if (data.start_date != null) {
                    params["start_date"] = date_to_naive_date(data.start_date);
                }

                if (data.end_date != null) {
                    params["end_date"] = date_to_naive_date(data.end_date);
                }

                set_search_params(params);
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