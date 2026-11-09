<?php
$current_page = isset($_GET['page']) ? $_GET['page'] : 'home';

// Version data that was previously in React state
$versions = [
    [
        'version' => '26.2',
        'image' => 'versions/26.2.jpg',
        'title' => 'Клиент'
    ]
];
?>

<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>FestVisuals</title>
    <link rel="stylesheet" href="styles.css">
    <script src="https://cdn.gpteng.co/gptengineer.js" type="module"></script>
</head>
<body>
    <div class="app">
        <!-- Sidebar -->
        <nav class="sidebar">
            <div class="logo">
                <h1>N</h1>
            </div>
            <a href="?page=home" class="nav-link <?php echo $current_page === 'home' ? 'active' : ''; ?>">
                <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>
            </a>
            <a href="?page=profile" class="nav-link <?php echo $current_page === 'profile' ? 'active' : ''; ?>">
                <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
            </a>
            <a href="?page=settings" class="nav-link <?php echo $current_page === 'settings' ? 'active' : ''; ?>">
                <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/></svg>
            </a>
            <button class="nav-link logout">
                <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/></svg>
            </button>
        </nav>

        <!-- Main Content -->
        <main class="content">
            <?php
            switch($current_page) {
                case 'home':
                    include 'pages/home.php';
                    break;
                case 'profile':
                    include 'pages/profile.php';
                    break;
                case 'settings':
                    include 'pages/settings.php';
                    break;
                default:
                    include 'pages/home.php';
            }
            ?>
        </main>
    </div>

    <!-- Version Card Modal -->
    <div id="versionModal" class="modal">
        <div class="modal-content">
            <div class="modal-header">
                <h2 id="modalVersion"></h2>
                <button class="close-button">НАЗАД <span>→</span></button>
            </div>
            <div class="modal-body">
                <img id="modalImage" src="" alt="">
                <div class="modal-info">
                    <h3 id="modalTitle"></h3>
                    <p>Мы создали для вас лучший клиент, который даст вам огромное преимущество в игре. В этом клиенте огромный функционал, который подойдёт под все популярные сервера майнкрафт. Этот клиент является стабильным, а это означает, что вы получите наилучший игровой опыт без багов и различных ошибок.</p>
                    <button class="launch-button">ЗАПУСТИТЬ</button>
                </div>
            </div>
        </div>
    </div>

    <script src="script.js"></script>
</body>
</html>