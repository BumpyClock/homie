`Homie` is a cross platform app that allows users to connect terminals on their machines remotely.

# Reference Projects
- agent-term ~/Projects/references/agent-term - Tauri app with a rust backend that is crossplatform. UI is in react + vite and backend is in rust.
- OpenClaw - ~/Projects/openclaw - an opensource AI assistant (Works mostly on Linux and macOS) written in TypeScript that provides a chat experience and a gateway architecture that allows seamless connections while routing through tailscale.
- vibetunnel - ~/Projects/references/vibetunnel - Terminal tunneling solution that works on Linux and macOS with a simple web based front-end.
- CodexMonitor ~/Projects/references/CodexMonitor - a Tauri v2 app built using the `codex app server` to give users a nice front end to the codex cli.
- craft-agent-oss- ~/Projects/references/craft-agent-oss - an electron app with a great and polished chat UX that we can take inspiration from for animations, microinteractions and other UI polish tasks. 

## Using reference projects 
All the projects have open source and permissible liceenses so you can copy their code and/or use it as a reference and guide.


# Goals

- Provide a cross platform solution that allows the user to connect back to their machines and run terminal sessions on their machine on the go. 
- Web page to access running terminals. 
- React Native apps for iOS and Android to provide on the go access.
- Local access. Remote access with auth. Tailscale for remote access. 
- Encrypted traffic if possible. 

# Important
- Overall plan is in ./plans/plan.md. Read it. Use reference projects for ideas on implementation.
- Corresponding plans for backend, web , and react native apps are in ./plans/phase1/<backend.prd, rn-mobile.prd, and web.prd>


