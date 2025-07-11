import { ArrowLeft } from "lucide-react";
import { useForm, SubmitHandler } from "react-hook-form";
import { useNavigate, Link } from "react-router-dom";

import { ApiError, req_api_json_empty } from "@/net";

import { Input, PasswordInput } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
    Form,
    FormControl,
    FormField,
    FormItem,
    FormLabel,
    FormMessage,
    FormRootError
} from "@/components/ui/form";
import { Separator } from "@/components/ui/separator";

interface RegisterForm {
    token: string,
    username: string,
    password: string,
    confirm: string,
}

export function Register() {
    const navigate = useNavigate();

    const form = useForm<RegisterForm>({
        defaultValues: {
            token: "",
            username: "",
            password: "",
            confirm: "",
        }
    });

    const on_submit: SubmitHandler<RegisterForm> = async (data, event) => {
        if (data.confirm !== data.password) {
            form.setError("confirm", {message: "Confirm does not match password"});

            return;
        }

        try {
            await req_api_json_empty("POST", "/register", data);

            navigate("/journals");
        } catch (err) {
            if (err instanceof ApiError) {
                switch (err.kind) {
                    case "InviteNotFound":
                        form.setError("token", {message: "The invite token was not found"});
                        break;
                    case "InviteUsed":
                        form.setError("token", {message: "The invite has already been used"});
                        break;
                    case "InviteExpired":
                        form.setError("token", {message: "The invite token has expired"});
                        break;
                    case "InvalidConfirm":
                        form.reset({"confirm": ""});
                        form.setError("confirm", {message: "Invalid confirm"});
                        break;
                    case "UsernameExists":
                        form.setError("username", {message: "Username already exists"});
                        break;
                    default:
                        form.setError("root", {message: "Server when sending registration"});
                        break;
                }
            } else {
                console.error("error when sending login:", err);

                form.setError("root", {message: "Client error when sending registration."});
            }
        }
    };

    return <div className="flex w-full h-full">
        <div className="mx-auto my-auto border rounded-lg">
            <Form {...form}>
                <form className="space-y-4 p-4" onSubmit={form.handleSubmit(on_submit)}>
                    <FormRootError/>
                    <FormField control={form.control} name="token" render={({field}) => {
                        return <FormItem>
                            <FormLabel>Register Token</FormLabel>
                            <FormControl>
                                <Input type="text" {...field}/>
                            </FormControl>
                            <FormMessage/>
                        </FormItem>;
                    }}/>
                    <FormField control={form.control} name="username" render={({field}) => {
                        return <FormItem>
                            <FormLabel>Username</FormLabel>
                            <FormControl>
                                <Input type="text" {...field}/>
                            </FormControl>
                            <FormMessage/>
                        </FormItem>;
                    }}/>
                    <FormField control={form.control} name="password" render={({field}) => {
                        return <FormItem>
                            <FormLabel>Password</FormLabel>
                            <FormControl>
                                <PasswordInput autoComplete="new-password" {...field}/>
                            </FormControl>
                            <FormMessage/>
                        </FormItem>;
                    }}/>
                    <FormField control={form.control} name="confirm" render={({field}) => {
                        return <FormItem>
                            <FormLabel>Confirm Password</FormLabel>
                            <FormControl>
                                <PasswordInput autoComplete="new-password" {...field}/>
                            </FormControl>
                            <FormMessage/>
                        </FormItem>;
                    }}/>
                    <div className="flex flex-row gap-x-4 justify-center">
                        <Button type="submit" disabled={form.formState.isSubmitting}>Sign Up</Button>
                    </div>
                </form>
            </Form>
            <Separator/>
            <div className="flex flex-row justify-center p-4">
                <Link to="/login">
                    <Button type="button" variant="secondary"><ArrowLeft/>Login</Button>
                </Link>
            </div>
        </div>
    </div>;
}
