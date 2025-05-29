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
import { UserList, AttachedUser } from "@/components/users";
import { RoleList, AttachedRole } from "@/components/roles";

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
    let res = await fetch(`/admin/groups/${id}`);

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

        let res = await fetch("/admin/groups", {
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

        let res = await fetch(`/admin/groups/${groups_id}`, {
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

                navigate(`/admin/groups/${created.id}`);
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
            let res = await fetch(`/admin/groups/${groups_id}`, {
                method: "DELETE",
            });

            switch (res.status) {
            case 200:
                navigate("/admin/groups");
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
