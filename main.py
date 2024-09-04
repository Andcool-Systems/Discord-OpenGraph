import aiohttp
from fastapi import FastAPI, Request
from fastapi.responses import HTMLResponse, JSONResponse, RedirectResponse
import uvicorn
from dotenv import load_dotenv
import os

load_dotenv()
app = FastAPI()

@app.get("/", response_class=RedirectResponse)
async def root():
    return RedirectResponse(url="https://github.com/Andcool-Systems/Discord-OpenGraph/tree/main")


@app.get("/{uid}")
async def uid(uid: str, request: Request):
    if uid == 'favicon.ico':
        return
    
    user_data = None
    async with aiohttp.ClientSession("https://discord.com") as session:
        async with session.get(f"/api/v10/users/{uid}/profile", headers={"Authorization": os.getenv("TOKEN")}) as response:
            if response.status == 200:
                user_data = (await response.json())['user']
        if not user_data:
            async with session.get(f"/api/v10/users/{uid}", headers={"Authorization": os.getenv("TOKEN")}) as response:
                if response.status == 200:
                    user_data = await response.json()

    if not user_data:
        return JSONResponse({
                'status': 'error',
                'message': 'user not found'
            }, 
            status_code=404
        )
    
    if request.headers.get('accept') == 'application/json':
        response_data = {
            "global_name": user_data.get('global_name', user_data.get('username')),
            "username": user_data.get('username'),
            "banner_color": user_data.get('banner_color', None),
            "avatar": f"https://cdn.discordapp.com/avatars/{uid}/{user_data.get('avatar')}",
            "bio": user_data.get('bio', None),
            "url": f"https://discord.com/users/{uid}"
        }
        return JSONResponse(content=response_data)
    
    html_content = f"""
    <!DOCTYPE html>
    <html>
    <head>
        <meta property="og:title" content="{user_data.get('global_name', user_data.get('username'))}">
        <meta name="theme-color" content="{user_data.get('banner_color', '#2563eb')}">
        <meta property="og:url" content="https://discord.com/users/{uid}" />
        <meta property="og:site_name" content="Discord" />
        <meta property="og:image" content="https://cdn.discordapp.com/avatars/{uid}/{user_data.get('avatar')}?size=1024" />
        <meta property="og:description" content="{user_data.get('bio', '')}" />
    </head>
    <body>
    </body>
    <script>
        window.location.replace("https://discord.com/users/{uid}");
    </script>
    </html>
    """
    
    return HTMLResponse(content=html_content)


if __name__ == "__main__":
    uvicorn.run(app, host="0.0.0.0", port=8765)
