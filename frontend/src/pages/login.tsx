import { EyeOff, Eye } from "lucide-react";
import { useState } from "react";
import { useForm, SubmitHandler } from "react-hook-form";
import { useNavigate, useLocation, Link } from "react-router-dom";

import { res_as_json } from "@/net";

import { Input, PasswordInput } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Form, FormControl, FormDescription, FormField, FormItem, FormLabel, FormMessage } from "@/components/ui/form";

interface LoginForm {
    username: string,
    password: string,
}

enum LoginFailure {
    UsernameNotFound = "UsernameNotFound",
    InvalidPassword = "InvalidPassword",
}

interface LoginSuccess {
    type: "Success"
}

interface LoginFailed {
    type: "Failure",
    value: LoginFailure
}

type LoginResult = LoginSuccess | LoginFailed;

async function send_login(given: LoginForm) {
    let body = JSON.stringify(given);
    let res = await fetch("/login", {
        method: "POST",
        headers: {
            "content-type": "application/json",
            "content-length": body.length.toString(10),
        },
        body
    });

    return await res_as_json<LoginResult>(res);
}

export function Login() {
    const navigate = useNavigate();
    const location = useLocation();

    const login_form = useForm<LoginForm>({
        defaultValues: {
            username: "",
            password: "",
        }
    });

    const [sending, setSending] = useState(false);
    const [show_password, set_show_password] = useState(false);

    const on_submit: SubmitHandler<LoginForm> = (data, event) => {
        setSending(true);

        send_login(data).then(result => {
            let prev = new URL(location.pathname + location.search, window.location.origin)
                .searchParams
                .get("prev");

            switch (result.type) {
                case "Success":
                    navigate(prev ?? "/journals");
                    break;
                case "Verify":
                    if (prev != null) {
                        navigate(`/verify?prev=${encodeURI(prev)}`);
                    } else {
                        navigate("/verify");
                    }

                    break;
                default:
                    console.log("login failed:", result.type);

                    setSending(false);

                    break;
            }
        }).catch(err => {
            console.error("error when sending login:", err);

            setSending(false);
        });
    };

    return <div className="flex w-full h-full">
        <div className="mx-auto my-auto">
            <Form {...login_form} children={
                <form className="space-y-4" onSubmit={login_form.handleSubmit(on_submit)}>
                    <FormField control={login_form.control} name="username" render={({field}) => {
                        return <FormItem>
                            <FormLabel>Username</FormLabel>
                            <FormControl>
                                <Input type="text" {...field}/>
                            </FormControl>
                        </FormItem>;
                    }}/>
                    <FormField control={login_form.control} name="password" render={({field}) => {
                        return <FormItem>
                            <FormLabel>Password</FormLabel>
                            <FormControl>
                                <PasswordInput
                                    autoComplete="current-password"
                                    {...field}
                                />
                            </FormControl>
                        </FormItem>;
                    }}/>
                    <div className="flex flex-row gap-x-4 justify-center">
                        <Link to="/register">
                            <Button type="button" variant="secondary">Register</Button>
                        </Link>
                        <Button type="submit">Login</Button>
                    </div>
                </form>
            }/>
        </div>
    </div>;
}
