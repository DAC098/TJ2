import { EyeOff, Eye } from "lucide-react";
import { useState } from "react";
import { useForm, SubmitHandler } from "react-hook-form";
import { useNavigate, useLocation } from "react-router-dom";

import { res_as_json } from "@/net";

import { Input } from "@/components/ui/input";
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

const Login = () => {
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
            if (result.type == "Success") {
                console.log("successful");

                let prev = new URL(location.pathname + location.search, window.location.origin)
                    .searchParams
                    .get("prev");

                if (prev != null) {
                    navigate(prev);
                } else {
                    navigate("/journals");
                }
            } else {
                console.log("login failed:", result.value);
            }
        }).catch(err => {
            console.error("error when sending login:", err);

            setSending(false);
        });
    };

    return <div className="flex w-full h-full">
        <div className="mx-auto my-auto">
            <Form {...login_form} children={
                <form onSubmit={login_form.handleSubmit(on_submit)}>
                    <FormField control={login_form.control} name="username" render={({field}) => {
                        return <FormItem>
                            <FormLabel>Username</FormLabel>
                            <FormControl>
                                <Input type="text" {...field}/>
                            </FormControl>
                            <FormMessage />
                        </FormItem>;
                    }}/>
                    <FormField control={login_form.control} name="password" render={({field}) => {
                        return <FormItem>
                            <FormLabel>Password</FormLabel>
                            <FormControl>
                                <div className="w-full relative">
                                    <Input
                                        type={show_password ? "text" : "password"}
                                        autoComplete="current-password"
                                        className="pr-10"
                                        {...field}
                                    />
                                    <Button
                                        type="button"
                                        variant="ghost"
                                        size="icon"
                                        className="absolute right-0 top-0"
                                        onClick={() => {
                                            set_show_password(v => (!v));
                                        }}
                                    >
                                        {show_password ? <EyeOff/> : <Eye/>}
                                    </Button>
                                </div>
                            </FormControl>
                            <FormMessage />
                        </FormItem>;
                    }}/>
                    <Button type="submit">Login</Button>
                </form>
            }/>
        </div>
    </div>
};

export default Login;
