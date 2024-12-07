import { format } from "date-fns";
import { Plus, Save, Trash, Eye, EyeOff, RefreshCcw, Search, Check } from "lucide-react";
import { useState, useEffect } from "react";
import { useForm, useFieldArray, useFormContext, FormProvider, SubmitHandler,  } from "react-hook-form";
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
import { GroupList, AttachedGroup } from "@/groups";
import { UserList, AttachedUser } from "@/users";

export interface RolePartial {
    id: number,
    uid: string,
    name: string,
    created: string,
    updated: string | null
}

export interface AttachedRole {
    role_id: number,
    name: string,
    added: string,
}

async function get_roles() {
    let res = await fetch("/roles");

    if (res.status !== 200) {
        console.log("non 200 response status:", res);

        return null;
    }

    return await res.json() as RolePartial[];
}

const columns: ColumnDef<RolePartial>[] = [
    {
        header: "Name",
        cell: ({ row }) => {
            return <Link to={`/roles/${row.original.id}`}>{row.original.name}</Link>;
        },
    },
    {
        header: "Mod",
        cell: ({ row }) => {
            return row.original.updated != null ? row.original.updated : row.original.created;
        },
    },
];

export function Roles() {
    let [loading, set_loading] = useState(false);
    let [roles, set_roles] = useState<RolePartial[]>([]);

    useEffect(() => {
        set_loading(true);

        get_roles().then(list => {
            if (list == null) {
                return;
            }

            set_roles(list);
        }).catch(err => {
            console.log("failed to load roles list", err);
        }).finally(() => {
            set_loading(false);
        });
    }, []);

    return <div className="max-w-3xl mx-auto my-auto space-y-4">
        <div className="flex flex-row flex-nowrap gap-x-4">
            <Link to="/roles/new">
                <Button type="button">New Role<Plus/></Button>
            </Link>
        </div>
        <DataTable columns={columns} data={roles}/>
    </div>;
}

interface RoleFull {
    id: number,
    uid: string,
    name: string,
    created: string,
    updated: string | null,
    permissions: AttachedPermission[],
    users: AttachedUser[],
    groups: AttachedGroup[],
}

interface AttachedPermission {
    scope: string,
    ability: string,
    added: string
}

interface RoleForm {
    name: string,
    permissions: AttachedPermission[],
    users: AttachedUser[],
    groups: AttachedGroup[],
}

async function get_role(role_id: string) {
    let res = await fetch(`/roles/${role_id}`);

    if (res.status !== 200) {
        return null;
    }

    return await res.json() as RoleFull;
}

interface RoleHeaderProps {
    role_id: string,
    on_delete: () => void,
}

function RoleHeader({role_id, on_delete}: RoleHeaderProps) {
    const form = useFormContext<RoleForm>();

    return <div className="flex flex-row flex-nowrap gap-x-4 items-center">
        <FormField control={form.control} name="name" render={({field}) => {
            return <FormItem className="w-1/2">
                <FormControl>
                    <Input type="text" placeholder="Name" {...field}/>
                </FormControl>
            </FormItem>
        }}/>
        <Button type="submit">Save<Save/></Button>
        {role_id !== "new" ?
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

export function Role() {
    const { role_id } = useParams();
    const navigate = useNavigate();

    if (role_id == null) {
        throw new Error("role_id is null");
    }

    const form = useForm<RoleForm>({
        defaultValues: async () => {
            let rtn = {
                name: "",
                permissions: [],
                users: [],
                groups: [],
            };

            if (role_id === "new") {
                return rtn;
            }

            try {
                let result = await get_role(role_id);

                if (result != null) {
                    rtn.name = result.name;
                    rtn.permissions = result.permissions;
                    rtn.users = result.users;
                    rtn.groups = result.groups;
                }
            } catch (err) {
                console.error("failed to retrieve role", err);
            }

            return rtn;
        }
    });

    const create_role = async (data: RoleForm) => {
        let body = JSON.stringify({
            name: data.name,
            permissions: [],
            users: data.users.map(attached => {
                return attached.users_id;
            }),
            groups: data.groups.map(attached => {
                return attached.groups_id;
            }),
        });

        let res = await fetch("/roles", {
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

            console.error("failed to create role", json);
            break;
        case 403:
            console.error("you do not have permission to create roles");
            break;
        default:
            console.warn("unhandled response status code");
            break;
        }

        return null;
    };

    const update_role = async (role_id: string, data: RoleForm) => {
        let body = JSON.stringify({
            name: data.name,
            permissions: [],
            users: data.users.map(attached => {
                return attached.users_id;
            }),
            groups: data.groups.map(attached => {
                return attached.groups_id;
            }),
        });

        let res = await fetch(`/roles/${role_id}`, {
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

            console.error("failed to update role", json);
            break;
        case 403:
            console.error("you do not have permission to update roles");
            break;
        case 404:
            console.error("role not found");
            break;
        default:
            console.warn("unhandled response status code");
            break;
        }

        return false;
    };

    const on_submit: SubmitHandler<RoleForm> = async (data, event) => {
        if (role_id === "new") {
            try {
                let created = await create_role(data);

                if (created == null) {
                    return;
                }

                form.reset({
                    name: created.name,
                    permissions: created.permissions,
                    users: created.users,
                    groups: created.groups,
                });

                navigate(`/roles/${created.id}`);
            } catch (err) {
                console.error("error when creating new role", err);
            }
        } else {
            try {
                if (await update_role(role_id, data)) {
                    form.reset(data);
                }
            } catch (err) {
                console.error("error when updating role", err);
            }
        }
    };

    const delete_role = async () => {
        if (role_id === "new") {
            return;
        }

        try {
            let res = await fetch(`/roles/${role_id}`, {
                method: "DELETE",
            });

            switch (res.status) {
            case 200:
                navigate("/roles");
                break;
            case 403:
                console.error("you do not have permission to delete roles");
                break;
            case 404:
                console.error("role not found");
                break;
            default:
                console.warn("unhandled response status code");
                break;
            }
        } catch (err) {
            console.error("error when deleting role", err);
        }
    };

    if (form.formState.isLoading) {
        return <div className="max-w-3xl mx-auto my-auto">
            Loading
        </div>;
    }

    return <div className="max-w-3xl mx-auto my-auto">
        <FormProvider<RoleForm> {...form} children={
            <form onSubmit={form.handleSubmit(on_submit)} className="space-y-4">
                <RoleHeader role_id={role_id} on_delete={delete_role}/>
                <Separator/>
                Permissions
                <Separator/>
                <div className="flex flex-row gap-x-4">
                    <UserList />
                    <GroupList />
                </div>
            </form>
        }/>

    </div>;
}

export function RoleList() {
    let form = useFormContext();
    let roles = useFieldArray({
        control: form.control,
        name: "roles",
    });

    let columns: ColumnDef<AttachedRole>[] = [
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
                        roles.remove(row.index);
                    }}
                >
                    <Trash/>
                </Button>;
            }
        }
    ];

    return <div className="grow space-y-4 basis-1/2">
        <div className="flex flex-row flex-nowrap gap-x-4 items-center">
            <span>Roles</span>
            <AddRoles on_added={new_role => {
                roles.append(new_role);
            }}/>
        </div>
        <DataTable columns={columns} data={(roles.fields as unknown) as AttachedRole[]}/>
    </div>;
}

interface AddRolesProps {
    on_added: (role: AttachedRole) => void,
}

function AddRoles({on_added}: AddRolesProps) {
    let [loading, set_loading] = useState(false);
    let [data, set_data] = useState<RolePartial[]>([]);

    let columns: ColumnDef<RolePartial>[] = [
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
                            role_id: row.original.id,
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
            let res = await fetch("/roles");

            if (res.status === 200) {
                let json = await res.json();

                set_data(json);
            }
        } catch (err) {
            console.error("failed to retrieve roles", err);
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
                Add Roles <Plus/>
            </Button>
        </SheetTrigger>
        <SheetContent>
            <SheetHeader>
                <SheetTitle>Add Roles</SheetTitle>
                <SheetDescription>
                    Add roles to the selected record
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
