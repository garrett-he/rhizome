import { invoke } from "@tauri-apps/api/core";
import { useCallback, useState } from "react";

export function useGreet() {
    const [message, setMessage] = useState("");

    const greet = useCallback(async (name: string) => {
        const msg = await invoke<string>("greet", { name });
        setMessage(msg);
    }, []);

    return { message, greet };
}
