import {  useEffect, useState } from "react";
import { Routes, Route, Link } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { EllipsisVertical, Pencil, Plus, RefreshCcw, Search, Trash } from "lucide-react";

import { CenterPage } from "@/components/ui/page";

import { Entries } from "@/pages/journals/journals_id/entries";
import { Entry } from "@/pages/journals/journals_id/entries/entries_id";
import { Journal } from "@/pages/journals/journals_id";
import { ApiError, req_api_json, req_api_json_empty } from "@/net";
import { JournalPartial } from "@/journals/api";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Input } from "@/components/ui/input";
import { FormProvider, SubmitHandler, useForm } from "react-hook-form";
import { FormControl, FormField, FormItem, FormLabel, FormMessage, FormRootError } from "@/components/ui/form";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { toast } from "sonner";
import { formatDistanceToNow } from "date-fns";
import { JournalShareEdit, JournalShareSearch } from "./journals/share";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { ApiErrorMsg, ErrorMsg } from "@/components/error";

export function JournalRoutes() {
    return <Routes>
        <Route index element={<JournalsIndex />}/>
        <Route path="/:journals_id" element={<Journal />}/>
        <Route path="/:journals_id/entries" element={<Entries />}/>
        <Route path="/:journals_id/entries/:entries_id" element={<Entry />}/>
        <Route path="/:journals_id/share" element={<JournalShareSearch/>}/>
        <Route path="/:journals_id/share/:share_id" element={<JournalShareEdit/>}/>
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
    const {data, isError, isLoading, refetch} = useQuery({
        queryKey: query_key,
        queryFn: async ({queryKey}) => {
            return await req_api_json<JournalPartial[]>("GET", "/journals");
        },
    });

    let content;

    if (isLoading) {
        content = <div>Loading...</div>;
    } else if (isError) {
        content = <div>Failed to load your journals.</div>;
    } else if (data == null) {
        content = <div>No data to display</div>;
    } else if (data.length === 0) {
        content = <div>No journals found</div>;
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
                    refetch();
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
                    <Button type="button"><Plus/>New Journal</Button>
                </Link>
                <JournalInvite/>
            </form>
        </FormProvider>
        <Separator/>
        {content}
    </CenterPage>;
}

interface JournalItemProps {
    journal: JournalPartial,
    view_only?: boolean
}

function JournalItem({journal, view_only = false}: JournalItemProps) {
    return <div className="rounded-lg border space-y-4">
        <div className="pt-4 px-4 flex flex-row flex-nowrap gap-x-4 items-center">
            {view_only ?
                <h2 className="text-2xl">{journal.name}</h2>
                :
                <Link to={`/journals/${journal.id}/entries`}>
                    <h2 className="text-2xl">{journal.name}</h2>
                </Link>
            }
            <div className="flex-grow"/>
            {view_only ?
                null
                :
                <>
                    <Link to={`/journals/${journal.id}/entries/new`}>
                        <Button type="button" variant="secondary" title="New Entry">
                            New Entry<Plus />
                        </Button>
                    </Link>
                    <JournalOptions journals_id={journal.id}/>
                </>
            }
        </div>
        <Separator/>
        <div className="px-4 pb-4 space-y-2">
            {journal.description != null ?
                <p className="text-base">{journal.description}</p>
                :
                null
            }
            <div className="flex flex-row items-center gap-x-4">
                <div className="text-sm">owner: {journal.owner.username}</div>
                {journal.updated != null ?
                    <div className="text-sm" title={journal.updated}>updated {formatDistanceToNow(journal.updated, {addSuffix: true, includeSeconds: true})}</div>
                    :
                    <div className="text-sm" title={journal.created}>created {formatDistanceToNow(journal.created, {addSuffix: true, includeSeconds: true})}</div>
                }
            </div>
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

function JournalInvite() {
    const [view_dialog, set_view_dialog] = useState(false);
    const [invite_code, set_invite_code] = useState("");

    const client = useQueryClient();
    const {data, error, isLoading, refetch} = useQuery({
        queryKey: ["journal_invite", invite_code] as ["journal_invite", string],
        queryFn: async ({queryKey}) => {
            return await req_api_json<JournalPartial>("GET", `/journals/invite/${queryKey[1]}`);
        },
        enabled: invite_code.length === 8,
        retry: false,
        retryOnMount: false,
        gcTime: 0
    });

    const {mutate, isPending, error: decide_error} = useMutation({
        mutationFn: async ({code, accept}: {code: string, accept: boolean}) => {
            await req_api_json_empty("PATCH", `/journals/invite/${code}`, {
                type: accept ? "Accept" : "Reject"
            });
        },
        onSuccess: (data, vars, ctx) => {
            if (vars.accept) {
                client.refetchQueries({
                    queryKey: ["journal_search"]
                });
            }

            set_invite_code("");
            set_view_dialog(false);
        },
        onError: (err, vars, ctx) => {
            console.error("failed to decide on journal invite:", err);
        }
    });

    return <Dialog open={view_dialog} onOpenChange={open => {
        if (!open) {
            set_invite_code("");
        }

        set_view_dialog(open);
    }}>
        <DialogTrigger asChild>
            <Button type="button" variant="secondary">
                <Plus/>Journal Invite
            </Button>
        </DialogTrigger>
        <DialogContent>
            <DialogHeader>
                <DialogTitle>Journal Invite</DialogTitle>
                <DialogDescription>
                    Add a new journal from an invite of another user.
                </DialogDescription>
            </DialogHeader>
            <RetrieveJournalInvite invite_code={invite_code} disabled={isLoading} on_lookup={code => {
                if (code === invite_code) {
                    refetch();
                } else {
                    set_invite_code(code);
                }
            }}/>
            {isLoading ?
                <div>Loading...</div>
                :
                null
            }
            <JournalInviteError err={error}/>
            <JournalInviteError err={decide_error}/>
            {data != null ?
                <>
                    <Separator/>
                    <JournalItem journal={data} view_only={true}/>
                    <DialogFooter>
                        <Button type="button" disabled={isPending} onClick={() => {
                            mutate({code: invite_code, accept: true});
                        }}>Accept</Button>
                        <Button type="button" variant={"destructive"} disabled={isPending} onClick={() => {
                            mutate({code: invite_code, accept: false});
                        }}>Reject</Button>
                    </DialogFooter>
                </>
                :
                null
            }
        </DialogContent>
    </Dialog>
}

interface RetrieveJournalInviteProps {
    invite_code: string,
    disabled: boolean,
    on_lookup: (code: string) => void,
}

function RetrieveJournalInvite({invite_code, disabled, on_lookup}: RetrieveJournalInviteProps) {
    const [curr_invite_code, set_curr_invite_code] = useState("");

    useEffect(() => {
        set_curr_invite_code(invite_code);
    }, [invite_code]);

    return <div className="flex flex-row gap-x-2 items-end">
        <div className="space-y-2 flex-grow">
            <Label>Invite Token</Label>
            <Input type="text" value={curr_invite_code} disabled={disabled} onChange={e => set_curr_invite_code(e.target.value)}/>
        </div>
        <Button
            type="button"
            disabled={disabled || curr_invite_code.length !== 8}
            onClick={() => on_lookup(curr_invite_code)}
        >
            <Search/>Find
        </Button>
    </div>
}

interface JournalInviteErrorProps {
    err: Error | null,
}

function JournalInviteError({err}: JournalInviteErrorProps) {
    if (err == null) {
        return null;
    }

    if (err instanceof ApiError) {
        switch (err.kind) {
            case "InviteNotFound":
                return <ErrorMsg title="Invite Not Found">
                    <p>The specified invite code was not found.</p>
                </ErrorMsg>;
            case "InviteExpired":
                return <ErrorMsg title="Invite Expired">
                    <p>The specified invite code has expired.</p>
                </ErrorMsg>;
            case "InviteUsed":
                return <ErrorMsg title="Invite Used">
                    <p>The specified invite code was already used.</p>
                </ErrorMsg>;
            default:
                return <ApiErrorMsg err={err}/>;
                break;
        }
    } else {
        return <ErrorMsg title="Client Error">
            <p>There was a client error when sending your request.</p>
        </ErrorMsg>;
    }
}