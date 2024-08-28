# Discord User Link OpenGraph Provider

For some reason, standard Discord user links do not have their own embed. Therefore, I created a provider for such links.

## Usage
Copy the user ID and paste it into the link `https://discord.andcool.ru/info?id=<uid>`. After that, you can send this link anywhere. In messengers, it will display your global nickname, profile description, and avatar. When clicking on the link, it will automatically redirect to the standard Discord user link.

### if you need a response in json
add this to headers: 
```http
"accept": "application/json"
```

## Examples
**Discord**  
![discord](/assets/discord.png)  

**Telegram**  
![telegram](/assets/telegram.png)  