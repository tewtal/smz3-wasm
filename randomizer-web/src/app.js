import React, { useRef } from 'react';
import { RandomizerClient, ConsoleInterface } from 'randomizer-client';

function App(props) {
    const sni_interface = useRef(new ConsoleInterface("sni"));
    const usb_interface = useRef(new ConsoleInterface("usb2snes"));
    const randomizer_client = useRef();

    const readMemoryTest = async (ci) => {
        const devices = await ci.list_devices();
        console.log(devices);
        for(let i = 0; i < 500; i++) {
            console.log("Write", i, await ci.write_multi(devices[0].uri, [0, 0x10], [new Uint8Array([0,1,2,3,4,5,6,7,8,9]), new Uint8Array([0,1,2,3,4,5,6,7,8,9])]));
            //console.log("Write", i, await ci.write(devices[0].uri, 0, new Uint8Array([0,1,2,3,4,5,6,7,8,9])));
        }
        
        //console.log("Read", ci, await ci.read_multi(devices[0].uri, [0, 0x20]));
    }

    const startSession = async () => {
        const client = randomizer_client.current ?? (randomizer_client.current = new RandomizerClient("https://localhost:7108", "144eac688e144d70bab61e5ab35de8fa"));
        console.log(await client.initialize());
        //console.log(await client.register_player(2));
        console.log(await client.login_player("00939972f1d444eda94b70d8806b686d"));
        //console.log(await client.get_patch());
        const devices = await client.list_devices();
        console.log(devices);
        if(devices.length > 0) {
            await client.start(devices[0].uri);
        }
    }

    return <>
        <h1>Hello World</h1>
        <button onClick={async () => await readMemoryTest(sni_interface.current)}>Test SNI</button>
        <button onClick={async () => await readMemoryTest(usb_interface.current)}>Test WebSocket</button>
        <button onClick={startSession}>Get Session Data</button>
    </>;
}

export default App;