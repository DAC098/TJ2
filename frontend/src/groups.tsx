import { format } from "date-fns";
import { Plus, Save, Trash, Eye, EyeOff, RefreshCcw, Search, Check } from "lucide-react";
import { useState, useEffect } from "react";
import { useForm, useFieldArray, useFormContext, FormProvider, SubmitHandler } from "react-hook-form";
import { Link, useParams, useNavigate } from "react-router-dom";

import { Button } from "@/components/ui/button";
import {
    FormControl,
    FormDescription,
    FormField,
    FormItem,
    FormLabel,
    FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
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
import { UserList, AttachedUser } from "@/users";
import { RoleList, AttachedRole } from "@/roles";

export interface GroupPartial {
    id: number,
    uid: string,
    name: string,
    created: string,
    updated: string | null
}

export interface AttachedGroup {
    groups_id: number,
    name: string,
    added: string
}

async function get_groups() {
    let res = await fetch("/groups");

    if (res.status !== 200) {
        console.log("non 200 response status:", res);

        return null;
    }

    return await res.json() as GroupPartial[];
}

const columns: ColumnDef<GroupPartial>[] = [
    {
        header: "Name",
        cell: ({ row }) => {
            return <Link to={`/groups/${row.original.id}`}>{row.original.name}</Link>;
        }
    },
    {
        header: "Mod",
        cell: ({ row }) => {
            return row.original.updated != null ? row.original.updated : row.original.created;
        }
    }
];

export function Groups() {
    let [loading, set_loading] = useState(false);
    let [groups, set_groups] = useState<GroupPartial[]>([]);

    useEffect(() => {
        set_loading(true);

        get_groups().then(list => {
            if (list == null) {
                return;
            }

            set_groups(list);
        }).catch(err => {
            console.error("failed to load group list", err);
        }).finally(() => {
            set_loading(false);
        });
    }, []);

    return <div className="max-w-3xl mx-auto my-auto space-y-4">
        <div className="flex flex-row flex-nowrap gap-x-4">
            <Link to="/groups/new">
                <Button type="button">New Group<Plus /></Button>
            </Link>
        </div>
        <DataTable columns={columns} data={groups}/>
    </div>;
}

interface GroupFull {
    id: number,
    uid: string,
    name: string,
    created: string,
    updated: string | null,
    users: AttachedUser[],
    roles: AttachedRole[],
}

interface GroupForm {
    name: string,
    users: AttachedUser[],
    roles: AttachedRole[],
}

async function get_group(id: string) {
    let res = await fetch(`/groups/${id}`);

    if (res.status !== 200) {
        return null;
    }

    return await res.json() as GroupFull;
}

interface GroupHeaderProps {
    groups_id: string | null,
    on_delete: () => void,
}

function GroupHeader({groups_id, on_delete}: GroupHeaderProps) {
    const form = useFormContext<GroupForm>();

    return <div className="flex flex-row flex-nowrap gap-x-4 items-center">
        <FormField control={form.control} name="name" render={({field}) => {
            return <FormItem className="w-1/2">
                <FormControl>
                    <Input type="text" placeholder="Name" {...field}/>
                </FormControl>
            </FormItem>
        }}/>
        <Button type="submit">Save<Save/></Button>
        {groups_id != null && groups_id != "new" ?
            <Button
                type="button"
                variant="destructive"
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

export function Group() {
    const { groups_id } = useParams();
    const navigate = useNavigate();

    const form = useForm<GroupForm>({
        defaultValues: async () => {
            let rtn = {
                name: "",
                users: [],
                roles: []
            };

            if (groups_id == null || groups_id === "new") {
                return rtn;
            }

            try {
                let result = await get_group(groups_id);

                if (result != null) {
                    rtn.name = result.name;
                    rtn.users = result.users;
                    rtn.roles = result.roles;
                }
            } catch (err) {
                console.error("failed to retrieve group", err);
            }

            return rtn;
        }
    });

    const create_group = async (data: GroupForm) => {
        let body = JSON.stringify({
            name: data.name,
            users: data.users.map(attached => {
                return attached.users_id;
            }),
            roles: data.roles.map(attached => {
                return attached.role_id;
            }),
        });

        let res = await fetch("/groups", {
            method: "POST",
            headers: {
                "content-type": "application/json",
                "content-length": body.length.toString(10),
            },
            body
        });

        switch (res.status) {
        case 200:
            return await res.json();
        case 400:
            let json = await res.json();

            console.error("failed to create group", json);
            break;
        case 403:
            console.error("you do not have permission to create groups");
            break;
        default:
            console.warn("unhandled response status code");
            break;
        }

        return null;
    };

    const update_group = async (groups_id: string, data: GroupForm) => {
        let body = JSON.stringify({
            name: data.name,
            users: data.users.map(attached => {
                return attached.users_id;
            }),
            roles: data.roles.map(attached => {
                return attached.role_id;
            }),
        });

        let res = await fetch(`/groups/${groups_id}`, {
            method: "PATCH",
            headers: {
                "content-type": "application/json",
                "content-length": body.length.toString(10),
            },
            body
        });

        switch (res.status) {
        case 200:
            return true;
        case 400:
            let json = await res.json();

            console.error("failed to update group", json);
            break;
        case 403:
            console.error("you do not have permission to update users");
            break;
        case 404:
            console.error("group not found");
            break;
        default:
            console.warn("unhandled response status code");
            break;
        }

        return false;
    };

    const on_submit: SubmitHandler<GroupForm> = async (data, event) => {
        if (groups_id === "new") {
            try {
                let created = await create_group(data);

                if (created == null) {
                    return;
                }

                form.reset({
                    name: created.name,
                    users: created.users,
                    roles: created.roles,
                });

                navigate(`/groups/${created.id}`);
            } catch (err) {
                console.error("error when creating new group", err);
            }
        } else {
            try {
                if (await update_group(groups_id, data)) {
                    form.reset(data);
                }
            } catch (err) {
                console.error("error when updating group", err);
            }
        }
    };

    const delete_group = async () => {
        if (groups_id === "new") {
            return;
        }

        try {
            let res = await fetch(`/groups/${groups_id}`, {
                method: "DELETE",
            });

            switch (res.status) {
            case 200:
                navigate("/groups");
                break;
            case 403:
                console.error("you do not have permission to delete groups");
                break;
            case 404:
                console.error("group not found");
                break;
            default:
                console.warn("unhandled response status code");
                break;
            }
        } catch (err) {
            console.error("error when deleting group", err);
        }
    }

    if (form.formState.isLoading) {
        return <div className="max-w-3xl mx-auto my-auto">
            Loading
        </div>;
    }

    return <div className="max-w-3xl mx-auto my-auto">
        <FormProvider<GroupForm> {...form} children={
            <form onSubmit={form.handleSubmit(on_submit)} className="space-y-4">
                <GroupHeader groups_id={groups_id} on_delete={() => {
                    delete_group();
                }}/>
                <Separator/>
                <div className="flex flex-row gap-x-4">
                    <UserList />
                    <RoleList />
                </div>
            </form>
        }/>
    </div>;
}

export function GroupList() {
    let form = useFormContext();
    let groups = useFieldArray({
        control: form.control,
        name: "groups",
    });

    let columns: ColumnDef<AttachedGroup>[] = [
        {
            accessorKey: "name",
            header: "Name",
        },
        {
            header: "Added",
            cell: ({ row }) => {
                let date = new Date(row.original.added);

                return format(date, "yyyy/MM/dd");
            }
        },
        {
            id: "drop",
            cell: ({ row }) => {
                return <Button
                    type="button"
                    variant="destructive"
                    size="icon"
                    onClick={() => {
                        groups.remove(row.index);
                    }}
                >
                    <Trash/>
                </Button>;
            }
        }
    ];

    return <div className="grow space-y-4 basis-1/2">
        <div className="flex flex-row flex-nowrap gap-x-4 items-center">
            <span>Groups</span>
            <AddGroups on_added={new_group => {
                groups.append(new_group);
            }}/>
        </div>
        <DataTable columns={columns} data={(groups.fields as unknown) as AttachedGroup[]}/>
    </div>
}

interface AddGroupsProps {
    on_added: (group: AttachedGroup) => void,
}

function AddGroups({on_added}: AddGroupsProps) {
    let [loading, set_loading] = useState(false);
    let [data, set_data] = useState<GroupPartial[]>([]);

    let columns: ColumnDef<GroupPartial>[] = [
        {
            accessorKey: "name",
            header: "Name",
        },
        {
            id: "selector",
            cell: ({ row }) => {
                return <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    onClick={() => {
                        on_added({
                            groups_id: row.original.id,
                            name: row.original.name,
                            added: (new Date()).toJSON(),
                        });
                    }}
                >
                    <Plus/>
                </Button>;
            }
        }
    ];

    const retrieve = async () => {
        if (loading) {
            return;
        }

        set_loading(true);

        try {
            let res = await fetch("/groups");

            if (res.status === 200) {
                let json = await res.json();

                set_data(json);
            }
        } catch (err) {
            console.error("failed to retrieve groups", err);
        }

        set_loading(false);
    };

    return <Sheet onOpenChange={value => {
        if (value) {
            retrieve();
        }
    }}>
        <SheetTrigger asChild>
            <Button type="button" variant="secondary">
                Add Groups <Plus/>
            </Button>
        </SheetTrigger>
        <SheetContent>
            <SheetHeader>
                <SheetTitle>Add Groups</SheetTitle>
                <SheetDescription>
                    Add groups to the selected record
                </SheetDescription>
            </SheetHeader>
            <div className="space-y-4 mt-4">
                <div className="flex flex-row flex-nowrap gap-x-4 items-center">
                    <div className="w-full relative">
                        <Input type="text" placeholder="Search" className="pr-10" disabled={loading}/>
                        <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            className="absolute right-0 top-0"
                            disabled={loading}
                            onClick={() => {
                                retrieve();
                            }}
                        >
                            <Search/>
                        </Button>
                    </div>
                    <Button type="button" variant="ghost" size="icon" onClick={() => {}}>
                        <RefreshCcw/>
                    </Button>
                </div>
                <DataTable columns={columns} data={data}/>
            </div>
        </SheetContent>
    </Sheet>
}
