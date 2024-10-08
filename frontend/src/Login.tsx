import { useState } from "react";
import { useForm, SubmitHandler } from "react-hook-form";
import { useNavigate, useLocation } from "react-router-dom";

import { res_as_json } from "./net";

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

    console.log(location);

    const login_form = useForm<LoginForm>({
        defaultValues: {
            username: "",
            password: "",
        }
    });

    const [sending, setSending] = useState(false);

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
                    navigate("/entries");
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
        <form className="mx-auto my-auto" onSubmit={login_form.handleSubmit(on_submit)}>
            <div>
                <div>
                    <label htmlFor="username">Username</label>
                </div>
                <input type="text" {...login_form.register("username")}/>
            </div>
            <div>
                <div>
                    <label htmlFor="password">Password</label>
                </div>
                <input type="password" {...login_form.register("password")}/>
            </div>
            <button type="submit">Login</button>
        </form>
    </div>
};

export default Login;
