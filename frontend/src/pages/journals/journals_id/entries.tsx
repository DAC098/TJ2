import { format, formatDistanceToNow } from "date-fns";
import { useState, useMemo } from "react";
import { Link, useParams } from "react-router-dom";
import { Plus, Search, ChevronUp, ChevronDown, CalendarIcon } from "lucide-react";
import { useForm, FormProvider, SubmitHandler,  } from "react-hook-form";

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
import { useQuery } from "@tanstack/react-query";
import { req_api_json } from "@/net";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Checkbox } from "@/components/ui/checkbox";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { cn } from "@/utils";
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

interface SearchQuery {
    name: string,
    date_start: {
        enabled: boolean,
        value: Date,
    },
    date_end: {
        enabled: boolean,
        value: Date,
    }
}

function search_entries_query_key(journals_id: string): ["search_entries", string] {
    return ["search_entries", journals_id];
}

export function Entries() {
    const { journals_id } = useParams();

    if (journals_id == null) {
        throw new Error("missing journals_id param");
    }

    const [search_query, set_search_query] = useState<SearchQuery>({
        name: "",
        date_start: {
            enabled: false,
            value: new Date(),
        },
        date_end: {
            enabled: false,
            value: new Date(),
        }
    });
    const {data, isFetching, error, refetch} = useQuery<SearchEntriesResults, Error, SearchEntriesResults, ReturnType<typeof search_entries_query_key>>({
        queryKey: search_entries_query_key(journals_id),
        queryFn: async ({queryKey}) => {
            return await req_api_json<SearchEntriesResults>("GET", `/journals/${queryKey[1]}/entries`);
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
            },
            {
                header: "Mod",
                cell: ({ row }) => {
                    let created = new Date(row.original.created);
                    let updated = row.original.updated != null ? new Date(row.original.updated) : null;

                    let distance = formatDistanceToNow(updated ?? created, {
                        addSuffix: true,
                        includeSeconds: true,
                    });

                    return <Tooltip>
                        <TooltipTrigger>
                            <span className="text-nowrap">{distance}</span>
                        </TooltipTrigger>
                        <TooltipContent>
                            <div className="grid grid-cols-[auto_1fr] gap-x-2 gap-y-1">
                                <span className="text-right">created:</span><span>{created.toString()}</span>
                                {updated != null ? <><span className="text-right">updated:</span><span>{updated.toString()}</span></> : null}
                            </div>
                        </TooltipContent>
                    </Tooltip>;
                }
            }
        );

        return columns;
    }, [custom_fields]);

    console.log("rendering entries view");

    return <CenterPage className="max-w-6xl">
        <SearchHeader
            query={search_query}
            journals_id={journals_id}
            is_fetching={isFetching}
            on_search={set_search_query}
            on_refetch={refetch}
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

function SearchHeader({query, journals_id, is_fetching, on_search, on_refetch}: SearchHeaderProps) {
    const [view_options, set_view_options] = useState(false);

    const form = useForm({
        defaultValues: query,
    });

    const on_submit: SubmitHandler<SearchQuery> = (data, ev) => {
        if (form.formState.isDirty) {
            on_search(data);
        } else {
            on_refetch();
        }
    };

    return <FormProvider {...form}>
        <form className="sticky top-0 bg-background z-10 py-4 border-b" onSubmit={form.handleSubmit(on_submit)}>
            <Collapsible open={view_options} className="space-y-4" onOpenChange={set_view_options}>
                <div className="flex flex-row flex-nowrap items-center gap-x-4">
                    <div className="w-1/2 relative">
                        <FormField control={form.control} name="name" render={({field}) => {
                            return <FormItem>
                                <FormControl>
                                    <Input
                                        type="text"
                                        placeholder="Search"
                                        className="pr-10"
                                        {...field}
                                        disabled={field.disabled || is_fetching}
                                    />
                                </FormControl>
                                <FormMessage/>
                            </FormItem>;
                        }}/>
                    </div>
                    <Button type="submit" variant="secondary" size="icon" disabled={is_fetching}>
                        <Search/>
                    </Button>
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
                    <div className="space-y-2">
                        <FormField control={form.control} name="date_start.enabled" render={({field}) => {
                            return <FormItem className="flex flex-row flex-nowrap items-center gap-x-2 space-y-0">
                                <FormControl>
                                    <Checkbox checked={field.value} onCheckedChange={field.onChange}/>
                                </FormControl>
                                <FormLabel>Start Date</FormLabel>
                            </FormItem>
                        }}/>
                        <FormField control={form.control} name="date_start.value" render={({field}) => {
                            let enabled = form.getValues("date_start.enabled");

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
                            </FormItem>
                        }}/>
                    </div>
                    <div  className="space-y-2">
                        <FormField control={form.control} name="date_end.enabled" render={({field}) => {
                            return <FormItem className="flex flex-row flex-nowrap items-center gap-x-2 space-y-0">
                                <FormControl>
                                    <Checkbox checked={field.value} onCheckedChange={field.onChange}/>
                                </FormControl>
                                <FormLabel>Start Date</FormLabel>
                            </FormItem>
                        }}/>
                        <FormField control={form.control} name="date_end.value" render={({field}) => {
                            let enabled = form.getValues("date_end.enabled");

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
                            </FormItem>
                        }}/>
                    </div>
                </CollapsibleContent>
            </Collapsible>
        </form>
    </FormProvider>;
}