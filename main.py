import aiohttp
from fastapi import FastAPI
from fastapi.responses import HTMLResponse, JSONResponse, RedirectResponse
import uvicorn
from dotenv import load_dotenv
import os

load_dotenv()
app = FastAPI()

@app.get("/", response_class=RedirectResponse)
async def root():
    return RedirectResponse(url="https://discord.com")


@app.get("/{uid}", response_class=HTMLResponse)
async def uid(uid: str):

    if (uid == 'favicon.ico'):
        return

    user_data = None
    async with aiohttp.ClientSession("https://discord.com") as session:
        async with session.get(f"/api/v10/users/{uid}/profile", headers={"Authorization": os.getenv("TOKEN")}) as response:
            if response.status == 200:
                user_data = await response.json()

    if not user_data:
        return JSONResponse({
                'status': 'error',
                'message': 'user not found'
            }, 
            status_code=404
        )
    
    html_content = f"""
    <!DOCTYPE html>
    <html>
    <head>
        <meta property="og:title" content="{user_data['user']['global_name'] if user_data['user']['global_name'] else user_data['user']['username']}">
        <meta name="theme-color" content="{user_data['user']['banner_color'] if user_data['user']['banner_color'] else '#2563eb'}">
        <meta property="og:url" content="https://discord.com/users/{uid}" />
        <meta property="og:site_name" content="Discord" />
        <meta property="og:image" content="https://cdn.discordapp.com/avatars/{uid}/{user_data['user']['avatar']}" />
        <meta property="og:description" content="{user_data['user']['bio']}" />
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