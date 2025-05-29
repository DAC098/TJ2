import { format } from "date-fns";
import { Plus, Save, Trash, RefreshCw, Search, Check, ArrowLeft, CalendarIcon } from "lucide-react";
import { useState, useEffect, useCallback } from "react";
import { useForm, useFieldArray, useFormContext, FormProvider, SubmitHandler } from "react-hook-form";
import { Link, useParams, useNavigate } from "react-router-dom";

import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
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
    Popover,
    PopoverContent,
    PopoverTrigger,
} from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import {
    Sheet,
    SheetContent,
    SheetDescription,
    SheetHeader,
    SheetTitle,
    SheetTrigger,
} from "@/components/ui/sheet";
import {
    DataTable,
    ColumnDef,
} from "@/components/ui/table";

enum InviteStatus {
    Pending = "Pending",
    Accepted = "Accepted",
    Rejected = "Rejected",
}

interface InvitePartial {
    token: string,
    name: string,
    issued_on: string,
    expires_on: string | null,
    status: InviteStatus,
}

interface InviteTableProps {

}

export function InviteTable({}: InviteTableProps) {
    const [loading, set_loading] = useState(false);
    const [invites, set_invites] = useState<InvitePartial[]>([]);

    const search_invites = async () => {
        set_loading(true);

        try {
            let res = await fetch(`/admin/invites`);

            switch (res.status) {
                case 200:
                    let json = await res.json();

                    set_invites(json);
                    break;
                default:
                    console.log("unhandled response status");
            }
        } catch (err) {
            console.error("error when requesting entries", err);
        }

        set_loading(false);
    };

    const columns: ColumnDef<InvitePartial>[] = [
        {
            header: "Token",
            cell: ({row}) => {
                return <Link to={`/admin/invites/${row.original.token}`}>
                    {row.original.token}
                </Link>;
            }
        },
        {
            header: "Name",
            accessorKey: "name",
        },
        {
            header: "Issued On",
            cell: ({row}) => {
                let date = new Date(row.original.issued_on);

                return <span title={date} className="text-nowrap">
                    {format(date, "yyyy/MM/dd")}
                </span>;
            }
        },
        {
            header: "Expires On",
            cell: ({row}) => {
                if (row.original.expires_on != null) {
                    let date = new Date(row.original.expires_on);

                    return <span title={date} className="text-nowrap">
                        {format(date, "yyyy/MM/dd")}
                    </span>;
                } else {
                    return <span className="text-nowrap">Never</span>;
                }
            }
        },
        {
            header: "Status",
            cell: ({row}) => {
                switch (row.original.status) {
                    case InviteStatus.Pending:
                        return <span className="text-nowrap">Pending</span>;
                    case InviteStatus.Accepted:
                        return <span className="text-nowrap">Accepted</span>;
                    case InviteStatus.Rejected:
                        return <span className="text-nowrap">Rejected</span>;
                }
            }
        },
    ];

    useEffect(() => {
        search_invites();
    }, []);

    return <CenterPage className="pt-4 max-w-6xl">
        <div className="flex flex-row flex-nowrap gap-x-4">
            <div className="w-1/2 relative">
                <Input
                    type="text"
                    placeholder="Search"
                    className="pr-10"
                    disabled={loading}
                />
                <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="absolute right-0 top-0"
                    disabled={loading}
                >
                    <Search/>
                </Button>
            </div>
            <Button
                type="button"
                variant="secondary"
                size="icon"
                disabled={loading}
                onClick={() => {
                    search_invites();
                }}>
                <RefreshCw />
            </Button>
            <Link to={`/admin/invites/new`}>
                <Button type="button"><Plus/>New Invite</Button>
            </Link>
        </div>
        <DataTable columns={columns} data={invites}/>
    </CenterPage>;
}

interface InviteUser {
    id: number,
    username: string,
    created: string,
    updated: string | null,
}

interface InviteExpire {
    enabled: boolean,
    date: string | Date,
}

interface InviteForm {
    token: string | null,
    name: string,
    issued_on: string | Date | null,
    expires_on: InviteExpire,
    status: InviteStatus,
    user: InviteUser | null,
}

async function retrieve_invite(
    token: string
) {
    if (token === "new") {
        return null;
    }

    try {
        let res = await fetch(`/admin/invites/${token}`);

        switch (res.status) {
            case 200: {
                let json = await res.json();
                json.issued_on = json.issued_on != null ? new Date(json.issued_on) : null;
                json.expires_on.date = new Date(json.expires_on.date);

                if (json.user != null) {
                    json.user.created = new Date(json.user.created);
                    json.user.updated = json.user.updated != null ?
                        new Date(json.user.updated) :
                        null;
                }

                return json;
            }
            default: {
                let json = await res.json();

                console.log("failed to retrieve invite", json);

                break;
            }
        }
    } catch (err) {
        console.log("failed to retrieve invite", err);
    }

    return null;
}

async function delete_invite(token: string) {
    let res = await fetch(`/admin/invites/${token}`, {
        method: "DELETE",
    });

    if (res.status === 200) {
        return true;
    } else {
        return false;
    }
}

interface InviteHeaderProps {
    token: string
}

function InviteHeader({token}: InviteHeaderProps) {
    const form = useFormContext<InviteForm>();
    const navigate = useNavigate();

    let status = form.getValues("status");
    let status_pending = status === InviteStatus.Pending;

    return <div className="top-0 sticky flex flex-row flex-nowrap gap-x-4 bg-backgroud border-b py-2">
        <Link to="/admin/invites">
            <Button type="button" variant="ghost" size="icon">
                <ArrowLeft/>
            </Button>
        </Link>
        <FormField control={form.control} name="name" render={({field}) => {
            return <FormItem>
                <FormControl>
                    <Input
                        placeholder="Name"
                        {...field}
                        disabled={!status_pending || field.disabled}
                    />
                </FormControl>
            </FormItem>
        }}/>
        {status_pending ?
            <Button type="submit">Save<Save/></Button>
            :
            null
        }
        {token !== "new" ?
            <Button
                type="button"
                variant="destructive"
                disabled={false}
                onClick={() => {
                    delete_invite(token).then(() => {
                        navigate("/admin/invites");
                    }).catch(err => {
                        console.error("failed to delete invite", err);
                    });
                }}
            >
                Delete<Trash/>
            </Button>
            :
            null
        }
    </div>
}

interface InviteProps {
}

export function Invite({}: InviteProps) {
    const { token } = useParams();
    const navigate = useNavigate();

    const form = useForm<InviteForm>({
        defaultValues: async () => {
            return await retrieve_invite(token) ?? {
                token: null,
                name: "",
                issued_on: null,
                expires_on: {
                    enabled: false,
                    date: new Date(),
                },
                status: InviteStatus.Pending,
                user: null,
            };
        },
        disabled: false
    });

    const on_submit = async (data) => {
        if (token === "new") {
            try {
                let body = JSON.stringify({
                    name: data.name,
                    expires_on: data.expires_on.enabled ? data.expires_on.date : null
                });

                let res = await fetch("/admin/invites", {
                    method: "POST",
                    headers: {
                        "content-type": "application/json",
                        "content-length": body.length.toString(10),
                    },
                    body
                });

                switch (res.status) {
                    case 201:
                        let json = await res.json();
                        json.issued_on = new Date(json.issued_on);
                        json.expires_on.date = new Date(json.expires_on.date);

                        form.reset(json);

                        navigate(`/admin/invites/${json.token}`);

                        break;
                    default:
                        console.log("unhandled response status");
                }
            } catch (err) {
                console.error("failed to create new invite", err);
            }
        } else {
            try {
                let body = JSON.stringify({
                    name: data.name,
                    expires_on: data.expires_on.enabled ? data.expires_on.date : null
                });

                let res = await fetch(`/admin/invites/${token}`, {
                    method: "PATCH",
                    headers: {
                        "content-type": "application/json",
                        "content-length": body.length.toString(10),
                    },
                    body
                });

                switch (res.status) {
                    case 200:
                        let json = await res.json();
                        json.issued_on = new Date(json.issued_on);
                        json.expires_on.date = new Date(json.expires_on.date);

                        form.reset(json);

                        break;
                    default:
                        console.log("unhandled response status");
                }
            } catch (err) {
                console.error("failed to update new invite", err);
            }
        }
    };

    if (form.formState.isLoading) {
        return <div className="max-w-3xl mx-auto my-auto">
            <div className="top-0 sticky flex flex-row flex-nowrap gap-x-4 bg-backgroud border-b py-2">
                <Link to="/admin/invites">
                    <Button type="button" variant="ghost" size="icon">
                        <ArrowLeft/>
                    </Button>
                </Link>
            </div>
        </div>;
    }

    const invite_status = form.getValues("status");
    const invite_user = form.getValues("user");

    return <div className="max-w-3xl mx-auto my-auto">
        <FormProvider<InviteForm> {...form} children={
            <form onSubmit={form.handleSubmit(on_submit)} className="space-y-4">
                <InviteHeader token={token}/>
                <div className="flex-none space-y-2">
                    <FormField control={form.control} name="expires_on.enabled" render={({field}) => {
                        let status_pending = form.getValues("status") === InviteStatus.Pending;

                        return <FormItem className="flex flex-row flex-nowrap items-center gap-x-2">
                            <FormControl>
                                <Checkbox
                                    checked={field.value ?? false}
                                    disabled={field.disabled || !status_pending}
                                    onCheckedChange={() => {
                                        field.onChange(!field.value);
                                    }}
                                />
                            </FormControl>
                            <FormLabel className="my-0">Expires On</FormLabel>
                        </FormItem>;
                    }}/>
                    <FormField control={form.control} name={"expires_on.date"} render={({field}) => {
                        let now = new Date();
                        let date_value = field.value == null ? new Date() : field.value;
                        let enabled = form.getValues("expires_on.enabled");
                        let status_pending = form.getValues("status") === InviteStatus.Pending;

                        return <FormItem>
                            <Popover>
                                <PopoverTrigger asChild>
                                    <FormControl>
                                        <Button
                                            variant="outline"
                                            className={"w-[280px] justify-start text-left front-normal"}
                                            disabled={!enabled || !status_pending}
                                        >
                                            {format(date_value, "yyyy/MM/dd")}
                                            <CalendarIcon className="mr-2 h-4 w-4"/>
                                        </Button>
                                    </FormControl>
                                </PopoverTrigger>
                                <PopoverContent className="w-auto p-0" aligh="start">
                                    <Calendar
                                        name={field.name}
                                        mode="single"
                                        selected={date_value}
                                        onBlur={field.onBlur}
                                        onSelect={field.onChange}
                                        disabled={(date) => {
                                            return date < now;
                                        }}
                                    />
                                </PopoverContent>
                            </Popover>
                        </FormItem>;
                    }}/>
                </div>
                <div>
                    <span>Status: </span>
                    <span>{invite_status}</span>
                </div>
                {invite_user != null ?
                    <div>
                        <div>
                            <span>Username: </span>
                            <Link to={`/admin/users/${invite_user.id}`}>{invite_user.username}</Link>
                        </div>
                        <div>Created: {format(invite_user.created, "yyyy/LL/dd HH:mm:ss")}</div>
                        <div>Updated: {invite_user.updated != null ? format(invite_user.updated, "yyyy/LL/dd HH:mm:ss") : null}</div>
                    </div>
                    :
                    null
                }
            </form>
        }/>
    </div>;
}
