import {  useState } from "react";
import { Routes, Route, Link } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { EllipsisVertical, Pencil, Plus, RefreshCcw, Search, Trash } from "lucide-react";

import { CenterPage } from "@/components/ui/page";

import { Entries } from "@/pages/journals/journals_id/entries";
import { Entry } from "@/pages/journals/journals_id/entries/entries_id";
import { Journal } from "@/pages/journals/journals_id";
import { ApiError, req_api_json } from "@/net";
import { JournalPartial } from "@/journals/api";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Input } from "@/components/ui/input";
import { FormProvider, useForm } from "react-hook-form";
import { FormControl, FormField, FormItem } from "@/components/ui/form";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { toast } from "sonner";
import { formatDistanceToNow } from "date-fns";

export function JournalRoutes() {
    return <Routes>
        <Route index element={<JournalsIndex />}/>
        <Route path="/:journals_id" element={<Journal />}/>
        <Route path="/:journals_id/entries" element={<Entries />}/>
        <Route path="/:journals_id/entries/:entries_id" element={<Entry />}/>
    </Routes>;
}

interface SearchFields {
    name: string
}

function JournalsIndex() {
    const form = useForm<SearchFields>({
        defaultValues: {
            name: ""
        }
    });

    const [query_key, set_query_key] = useState(["journal_search", form.getValues()]);

    const client = useQueryClient();
    const {data, isError, isLoading} = useQuery({
        queryKey: query_key,
        initialData: [],
        queryFn: async ({queryKey}) => {
            console.log("sending query", queryKey);

            return await req_api_json<JournalPartial[]>("GET", "/journals");
        },
    });

    let content;

    if (data.length === 0) {
        if (isLoading) {
            content = <div>Loading...</div>;
        } else if (isError) {
            content = <div>Failed to load your journals.</div>;
        } else {
            content= <div>No journals to display</div>;
        }
    } else {
        content = <div className="space-y-4">
            {data.map(journal => <JournalItem key={journal.id} journal={journal}/>)}
        </div>;
    }

    return <CenterPage className="pt-4">
        <FormProvider<SearchFields> {...form}>
            <form className="flex flex-row flex-nowrap gap-x-4" onSubmit={form.handleSubmit((data, ev) => {
                if (form.formState.isDirty) {
                    form.reset(data);

                    set_query_key(["journal_search", data]);
                } else {
                    client.refetchQueries({
                        queryKey: ["journal_search", data],
                        exact: true,
                    });
                }
            })}>
                <div className="w-1/2 relative">
                    <FormField control={form.control} name="name" render={({field}) => {
                        return <FormItem className="w-full">
                            <FormControl>
                                <Input
                                    type="text"
                                    placeholder="search journals"
                                    className=""
                                    {...field}
                                    disabled={isLoading}
                                />
                            </FormControl>
                        </FormItem>;
                    }}/>
                    <Button
                        type="submit"
                        variant="ghost"
                        size="icon"
                        className="absolute right-0 top-0"
                        disabled={isLoading}
                    >
                        <Search />
                    </Button>
                </div>
                <Link to="/journals/new">
                    <Button type="button">New Journal<Plus/></Button>
                </Link>
            </form>
        </FormProvider>
        <Separator/>
        {content}
    </CenterPage>;
}

interface JournalItemProps {
    journal: JournalPartial
}

function JournalItem({journal}: JournalItemProps) {
    return <div className="rounded-lg border space-y-4">
        <div className="pt-4 px-4 flex flex-row flex-nowrap gap-x-4 items-center">
            <Link to={`/journals/${journal.id}/entries`}>
                <h2 className="text-2xl">{journal.name}</h2>
            </Link>
            <div className="flex-grow"/>
            <Link to={`/journals/${journal.id}/entries/new`}>
                <Button type="button" variant="secondary" title="New Entry">
                    New Entry<Plus />
                </Button>
            </Link>
            <JournalOptions journals_id={journal.id}/>
        </div>
        <Separator/>
        <div className="px-4 pb-4 space-y-2">
            {journal.description != null ?
                <p className="text-base">{journal.description}</p>
                :
                null
            }
            {journal.updated != null ?
                <span className="text-sm" title={journal.updated}>updated {formatDistanceToNow(journal.updated, {addSuffix: true, includeSeconds: true})}</span>
                :
                <span className="text-sm" title={journal.created}>created {formatDistanceToNow(journal.created, {addSuffix: true, includeSeconds: true})}</span>
            }
        </div>
    </div>
}

interface JournalOptionsProps {
    journals_id: number
}

type SyncResult = {
    type: "Noop"
} | {
    type: "Queued",
    successful: string[]
}

function JournalOptions({journals_id}: JournalOptionsProps) {
    const {mutate, isPending} = useMutation({
        mutationKey: ["sync_journal", journals_id],
        mutationFn: async () => {
            return await req_api_json<SyncResult>("POST", `/journals/${journals_id}/sync`, {});
        },
        onSuccess: (data) => {
            if (data.type === "Noop") {
                toast("No clients to sync journal");
            } else if (data.type === "Queued") {
                toast(`Syncing journal for clients: ${data.successful.join(", ")}`);
            }
        },
        onError: (err) => {
            if (err instanceof ApiError) {
                toast(`Failed to sync journal: ${err.kind}`);
            } else {
                console.error("failed to sync journal", err);

                toast(`Failed to sync journal`);
            }
        }
    });

    return <DropdownMenu>
        <DropdownMenuTrigger asChild>
            <Button type="button" variant="ghost" size="icon">
                <EllipsisVertical />
            </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent>
            <Link to={`/journals/${journals_id}`}>
                <DropdownMenuItem>
                    <Pencil />Edit
                </DropdownMenuItem>
            </Link>
            <DropdownMenuItem disabled={isPending} onClick={() => mutate()}>
                <RefreshCcw/>Synchronize
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem>
                <Trash/>Delete
            </DropdownMenuItem>
        </DropdownMenuContent>
    </DropdownMenu>;
}