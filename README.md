#movieGameBackend
This is a backend for a simple "guess the movie game".

The backend is built in Rust, and makes API requests to TMDb, processes the data, and sends this data in JSON format back to the front end.
Processing includes:
 - Filtering out obscure/inappropriate movies
 - Add the top 3 cast members names, so that they can be used as hints in the game.
    - the same for the director.

Checkout the hosted front end [[here | https://oliverheffernan.github.io/movieGame/dist/index.html]]
Or checkout the front end's source code [[here | https://github.com/OliverHeffernan/movieGame]]
