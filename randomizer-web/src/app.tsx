import React, { useCallback, useRef, useState } from 'react';
import { RandomizerClient, ConsoleInterface, Message } from 'randomizer-client';

enum ConnectionState {
    Stopped,
    Initialized,
    Authenticated,
    DeviceList,
    Running
}

function App(props: any) {
    const randomizer_client = useRef<RandomizerClient>(null);
    const [state, setState] = useState<ConnectionState>(ConnectionState.Stopped);
    const [errorMsg, setErrorMsg] = useState<null | string>("");
    const [deviceList, setDeviceList] = useState<any>();
    const [updateHandle, setUpdateHandle] = useState<any>();
    
    const handleMessage = useCallback(async (message, args) => {
        switch(message) {
            case Message.ConsoleReconnecting:
                console.log("Console reconnecting");
                break;
            case Message.ConsoleDisconnected:
                console.log("Console disconnected");
                break;
            case Message.ConsoleConnected:
                console.log("Console connected to: ", args);
                break;
        }
    }, []);


    const initializeSession = async () => {
        const client = randomizer_client.current ?? (randomizer_client.current = new RandomizerClient("https://localhost:7108", "9c1774a1a9e5482caa08ac4213ac75a3", handleMessage));
        try {
            await client.initialize();
            setState(ConnectionState.Initialized);
            setErrorMsg(null);
        } catch (e) {
            console.log("Could not initialize session");
            setErrorMsg(e);
        }
    }

    const loginSession = async () => {
        const client = randomizer_client.current;
        try {
            await client.login_player("8bb3d2add5124dfb91e64109a45c7967");
            //await client.register_player(0);
            setState(ConnectionState.Authenticated);
            setErrorMsg(null);
        } catch (e) {
            setErrorMsg(e);
        }
    }

    const listDevices = async () => {
        const client = randomizer_client.current;
        try {
            const devices = await client.list_devices();
            console.log(devices);
            setDeviceList(devices);
            setState(ConnectionState.DeviceList);
            setErrorMsg(null);
        } catch (e) {
            setErrorMsg(e);
        }
    }

    const startSession = async (deviceUri: string) => {
        const client = randomizer_client.current;
        
        if(updateHandle) {
            clearTimeout(updateHandle);
            setUpdateHandle(null);
        }

        try {
            await client.start(deviceUri);
            await updateSession();
            setState(ConnectionState.Running);
            setErrorMsg(null);
        } catch (e) {
            setErrorMsg(e);
        }
    }

    const updateSession = async () => {
        const client = randomizer_client.current;
        
        try {
            await client.update();
        } catch (e) {
            // The RandomizerClient will attempt to fix most error conditions by itself, so
            // we'll just log the error to the console and call update again later.
            // Any important status updates will be sent through the message callback
            console.log("Update error:", e);
        }

        setUpdateHandle(setTimeout(async () => { await updateSession(); }, 1000));
    }

    return <>
        <h1>Hello World!</h1>
        <h2>State: {ConnectionState[state]}</h2>
        <p>{errorMsg}</p>
        {state >= ConnectionState.Stopped ?
            <div>
                <hr />
                <p>Let's get initialized</p>
                <button onClick={initializeSession}>Initialize Session</button>
                <hr />
            </div>
        : ""}
        {state >= ConnectionState.Initialized ?
            <div>
                <hr />
                <p>Let's get logged in</p>
                <button onClick={loginSession}>Log in to session</button>
                <hr />
            </div>
        : ""}
        {state >= ConnectionState.Authenticated ?
            <div>
                <hr />
                <p>Let's list some devices</p>
                <button onClick={listDevices}>List devices</button>
                <hr />
            </div>
        : ""}
        {state >= ConnectionState.DeviceList ?
            <div>
                <hr />
                <p>Pick a device and let's go</p>
                {deviceList.map((device: any, index: any) => (
                    <div key={index}><p>Device: {device.uri}</p><button onClick={async () => startSession(device.uri)}>Start the fun</button></div>
                ))}                
                <hr />
            </div>
        : ""}    
        {state >= ConnectionState.Running ?
        <div>
            <hr />
            <h1>Service is up and running</h1>
        </div>
        : ""}

    </>;
}

export default App;