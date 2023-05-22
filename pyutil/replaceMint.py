import base64
import base58
import json

MOCK_MINTER_KEY = [109,3,86,101,96,42,254,204,98,232,34,172,105,37,112,24,223,194,66,133,2,105,54,228,54,97,90,111,253,35,245,73,93,83,136,36,51,237,111,8,250,149,126,98,135,211,138,191,207,116,66,179,204,231,147,190,217,190,220,93,181,102,164,238]

MINT_AUTHORITY = "7HJnvkjwb5PV8NsM2qrUNH7KDYN1dQQkXyFXbfbptEBo"
JSON_FILES = ['usdc', 'wbtc']

for file in JSON_FILES:
    original = json.load(open(f"{file}.json"))
    data = bytearray(base64.b64decode(original['account']['data'][0]))
    data[4:4+32] = base58.b58decode(f"{MINT_AUTHORITY}")
    print(base64.b64encode(data))
    original['account']['data'][0] = base64.b64encode(data).decode('utf8')
    json.dump(original, open(f"{file}-mock.json", 'w'))